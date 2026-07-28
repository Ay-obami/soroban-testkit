use std::panic;
use std::process::Command;

use clap::Args;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::xdr::{ScSpecEntry, ScSpecFunctionInputV0, ScSpecFunctionV0, ScSpecTypeDef};
use soroban_sdk::{Address, Env, IntoVal, Symbol, Val, Vec as SVec};

use super::CliError;

/// Arguments for `soroban-testkit limits`.
#[derive(Args)]
pub struct LimitsArgs {
    /// Path to the compiled contract .wasm file.
    #[arg(long, value_name = "PATH")]
    contract: std::path::PathBuf,
    /// The contract function to ramp.
    #[arg(long = "fn", value_name = "NAME")]
    function: String,
    /// The parameter to increase on each attempt.
    #[arg(long, value_name = "PARAM")]
    ramp: String,
}

/// Hidden: runs exactly one probe (a single ramp value) and reports its
/// outcome via exit code. See [`run`]'s doc comment for why this exists
/// as a separate process rather than an in-process call.
#[derive(Args)]
pub struct ProbeArgs {
    #[arg(long)]
    contract: std::path::PathBuf,
    #[arg(long = "fn")]
    function: String,
    #[arg(long)]
    ramp: String,
    #[arg(long)]
    value: u32,
}

/// Empirically discovers a resource ceiling by invoking `--fn` with an
/// increasing `--ramp` parameter — natively, with no network access —
/// until it fails, then reports the last successful value plus the CPU
/// instructions and memory consumed at that point.
///
/// # Why this spawns a subprocess per attempt
///
/// Ramping deliberately drives a real, compiled `.wasm` contract past its
/// resource limits. Verified empirically: when that failure happens
/// *during actual WASM execution* (as opposed to a native, non-WASM
/// `#[contract]` registration), the host can abort the whole process
/// (`thread caused non-unwinding panic. aborting.`) rather than return an
/// error or unwind — neither `catch_unwind` nor `try_invoke_contract`
/// (the SDK's own no-panic call path) prevents this for the WASM target.
/// So each attempt runs in its own child process (`soroban-testkit
/// __limits-probe`, hidden from `--help`): if that child aborts, only the
/// child dies, and the exit code alone tells this command whether that
/// ramp value succeeded.
///
/// # Scope
///
/// The ramp parameter must be a numeric type (`u32`/`i32`/`u64`/`i64`/
/// `u128`/`i128`, ramped as the value itself) or `Vec<Address>` (ramped as
/// the number of generated addresses) — this covers the common "maximum
/// recipients in a batch operation" question directly. Every other
/// parameter is filled with a fixed default (an address, `0`, an empty
/// collection, ...). A parameter type this command doesn't know how to
/// default (`Map`, `Tuple`, `Option`, `Result`, a user-defined type, ...)
/// is reported as an error rather than guessed at.
///
/// Ledger read/write counts and transaction size are **not** reported:
/// they come from a transaction's simulated resource footprint, which
/// this command's native invocation does not produce (that requires the
/// network-dependent `stellar contract invoke` simulation path, which
/// this crate deliberately avoids — see its "zero network access"
/// constraint). Only CPU instructions and memory, available locally via
/// the host's budget, are reported.
pub fn run(args: LimitsArgs) -> Result<(), CliError> {
    // Validate the configuration (function exists, ramp parameter exists,
    // every parameter's type is one this command knows how to default or
    // ramp) up front, in-process, before spawning any probes — this is
    // just type/spec checking, not invocation, so it carries none of the
    // abort risk documented above, and it means a typo'd --fn or --ramp
    // reports its actual cause instead of the generic "failed at value 1"
    // a swallowed child-process error would otherwise produce.
    let (_, env, function, ramp_index) = load(&args.contract, &args.function, &args.ramp)?;
    build_args(&env, &function, ramp_index, 1)?;
    drop(env);

    let self_exe = std::env::current_exe()
        .map_err(|err| CliError(format!("failed to locate this binary: {err}")))?;

    let probe = |value: u32| -> Result<bool, CliError> {
        let status = Command::new(&self_exe)
            .arg("__limits-probe")
            .arg("--contract")
            .arg(&args.contract)
            .arg("--fn")
            .arg(&args.function)
            .arg("--ramp")
            .arg(&args.ramp)
            .arg("--value")
            .arg(value.to_string())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map_err(|err| CliError(format!("failed to spawn probe: {err}")))?;
        Ok(status.success())
    };

    // Find a failing upper bound by doubling.
    let mut last_ok: Option<u32> = None;
    let mut low = 1u32;
    let mut high = None;
    loop {
        if probe(low)? {
            last_ok = Some(low);
            match low.checked_mul(2) {
                Some(next) => low = next,
                None => {
                    high = None;
                    break;
                }
            }
        } else {
            high = Some(low);
            break;
        }
        if low > 1 << 24 {
            break;
        }
    }

    let Some(last_ok) = last_ok else {
        return Err(CliError(format!(
            "{:?} failed even at the smallest ramp value (1) for parameter {:?}",
            args.function, args.ramp
        )));
    };

    // Binary search between the last success and the first failure for a
    // tighter bound, if we found one.
    let mut best = last_ok;
    if let Some(hi_start) = high {
        let mut lo = last_ok;
        let mut hi = hi_start;
        while lo + 1 < hi {
            let mid = lo + (hi - lo) / 2;
            if probe(mid)? {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        best = lo;
    }

    // Measure resources at the best-known-good value, in-process this
    // time (a known-successful call is safe to make directly).
    let (instructions, memory_bytes) = measure(&args, best)?;

    println!(
        "last successful {} for {:?}: {best}",
        args.ramp, args.function
    );
    println!("  instructions: {instructions}");
    println!("  memory bytes: {memory_bytes}");
    println!(
        "  (ledger reads/writes and transaction size are not measured by this command; \
         see --help)"
    );

    Ok(())
}

/// The hidden probe entry point: performs exactly one invocation and
/// returns `Ok(())` (exit 0) or `Err` (exit 1) — no measurement, no
/// output. Run in its own process by [`run`].
pub fn run_probe(args: ProbeArgs) -> Result<(), CliError> {
    let _quiet = QuietPanics::install();
    let (contract_id, env, function, ramp_index) =
        load(&args.contract, &args.function, &args.ramp)?;
    let args_vec = build_args(&env, &function, ramp_index, args.value)?;
    let func = Symbol::new(&env, &args.function);
    if invoke_succeeds(&env, &contract_id, &func, args_vec) {
        Ok(())
    } else {
        Err(CliError("probe failed".to_string()))
    }
}

fn measure(args: &LimitsArgs, value: u32) -> Result<(u64, u64), CliError> {
    let (contract_id, env, function, ramp_index) =
        load(&args.contract, &args.function, &args.ramp)?;
    let func = Symbol::new(&env, &args.function);
    let mut budget = env.cost_estimate().budget();
    budget.reset_default();
    let args_vec = build_args(&env, &function, ramp_index, value)?;
    if !invoke_succeeds(&env, &contract_id, &func, args_vec) {
        return Err(CliError(
            "internal error: the ramp value chosen as successful failed on re-measurement"
                .to_string(),
        ));
    }
    Ok((budget.cpu_instruction_cost(), budget.memory_bytes_cost()))
}

/// Silences the default panic hook for the duration it's held, restoring
/// the previous hook on drop, so a probe's failure doesn't dump a panic
/// message and the host's diagnostic event log to stderr.
type PanicHook = Box<dyn Fn(&panic::PanicHookInfo<'_>) + Sync + Send>;

struct QuietPanics {
    previous: Option<PanicHook>,
}

impl QuietPanics {
    fn install() -> Self {
        let previous = panic::take_hook();
        panic::set_hook(Box::new(|_| {}));
        Self {
            previous: Some(previous),
        }
    }
}

impl Drop for QuietPanics {
    fn drop(&mut self) {
        if let Some(hook) = self.previous.take() {
            panic::set_hook(hook);
        }
    }
}

fn load(
    contract: &std::path::Path,
    function: &str,
    ramp: &str,
) -> Result<(Address, Env, ScSpecFunctionV0, usize), CliError> {
    let wasm = std::fs::read(contract)
        .map_err(|err| CliError(format!("failed to read {}: {err}", contract.display())))?;

    let entries = soroban_spec::read::from_wasm(&wasm)
        .map_err(|err| CliError(format!("failed to read contract spec: {err}")))?;

    let func_spec = entries
        .into_iter()
        .find_map(|entry| match entry {
            ScSpecEntry::FunctionV0(f) if f.name.to_string() == function => Some(f),
            _ => None,
        })
        .ok_or_else(|| {
            CliError(format!(
                "no function named {function:?} in {}'s spec",
                contract.display()
            ))
        })?;

    let ramp_index = func_spec
        .inputs
        .iter()
        .position(|input| input.name.to_utf8_string_lossy() == ramp)
        .ok_or_else(|| CliError(format!("{function:?} has no parameter named {ramp:?}")))?;

    let env = Env::new_with_config(soroban_sdk::testutils::EnvTestConfig {
        capture_snapshot_at_drop: false,
    });
    let contract_id = env.register(wasm.as_slice(), ());

    Ok((contract_id, env, func_spec, ramp_index))
}

fn build_args(
    env: &Env,
    function: &ScSpecFunctionV0,
    ramp_index: usize,
    ramp_value: u32,
) -> Result<SVec<Val>, CliError> {
    let admin = Address::generate(env);
    let mut vals = SVec::new(env);
    for (i, input) in function.inputs.iter().enumerate() {
        let val = if i == ramp_index {
            ramp_val(env, &input.type_, ramp_value)?
        } else {
            default_val(env, &admin, &input.type_, input)?
        };
        vals.push_back(val);
    }
    Ok(vals)
}

fn invoke_succeeds(env: &Env, contract_id: &Address, func: &Symbol, args: SVec<Val>) -> bool {
    // try_invoke_contract is the SDK's own no-panic call path, used here
    // so a probe that succeeds doesn't need catch_unwind at all — only a
    // *failing* real-WASM call risks the process abort documented above,
    // and that only ever happens in the isolated child process.
    matches!(
        env.try_invoke_contract::<Val, soroban_sdk::Error>(contract_id, func, args),
        Ok(Ok(_))
    )
}

fn ramp_val(env: &Env, type_: &ScSpecTypeDef, ramp_value: u32) -> Result<Val, CliError> {
    match type_ {
        ScSpecTypeDef::U32 => Ok(ramp_value.into_val(env)),
        ScSpecTypeDef::I32 => Ok((ramp_value as i32).into_val(env)),
        ScSpecTypeDef::U64 => Ok((ramp_value as u64).into_val(env)),
        ScSpecTypeDef::I64 => Ok((ramp_value as i64).into_val(env)),
        ScSpecTypeDef::U128 => Ok((ramp_value as u128).into_val(env)),
        ScSpecTypeDef::I128 => Ok((ramp_value as i128).into_val(env)),
        ScSpecTypeDef::Vec(inner) if matches!(*inner.element_type, ScSpecTypeDef::Address) => {
            let mut addrs = SVec::new(env);
            for _ in 0..ramp_value {
                addrs.push_back(Address::generate(env));
            }
            Ok(addrs.into_val(env))
        }
        other => Err(CliError(format!(
            "the ramp parameter's type ({other:?}) isn't supported yet; supported ramp types \
             are u32/i32/u64/i64/u128/i128 and Vec<Address>"
        ))),
    }
}

fn default_val(
    env: &Env,
    admin: &Address,
    type_: &ScSpecTypeDef,
    input: &ScSpecFunctionInputV0,
) -> Result<Val, CliError> {
    match type_ {
        ScSpecTypeDef::Address => Ok(admin.into_val(env)),
        ScSpecTypeDef::Bool => Ok(false.into_val(env)),
        ScSpecTypeDef::U32 => Ok(0u32.into_val(env)),
        ScSpecTypeDef::I32 => Ok(0i32.into_val(env)),
        ScSpecTypeDef::U64 => Ok(0u64.into_val(env)),
        ScSpecTypeDef::I64 => Ok(0i64.into_val(env)),
        ScSpecTypeDef::U128 => Ok(0u128.into_val(env)),
        ScSpecTypeDef::I128 => Ok(0i128.into_val(env)),
        ScSpecTypeDef::Symbol => Ok(Symbol::new(env, "x").into_val(env)),
        ScSpecTypeDef::String => Ok(soroban_sdk::String::from_str(env, "").into_val(env)),
        ScSpecTypeDef::Bytes => Ok(soroban_sdk::Bytes::new(env).into_val(env)),
        ScSpecTypeDef::Vec(_) => Ok(SVec::<Val>::new(env).into_val(env)),
        other => Err(CliError(format!(
            "parameter {:?} has type {other:?}, which this command doesn't know how to \
             default yet; only the --ramp parameter needs a type this command understands \
             today, every other parameter needs a supported default type",
            input.name.to_utf8_string_lossy()
        ))),
    }
}
