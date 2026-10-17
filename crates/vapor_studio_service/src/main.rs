use std::{env, path::PathBuf, process};
use vapor_studio_service::{ServiceBootstrap, run_simulator, run_stdio_service};

fn main() {
    let mut args = env::args_os().skip(1);
    let first = args.next();
    if first.as_deref().is_some_and(|arg| arg == "simulate") {
        let mut scenario = None;
        while let Some(argument) = args.next() {
            if argument == "--scenario" {
                scenario = args.next().map(PathBuf::from);
            } else if argument == "--help" || argument == "-h" {
                println!("vapor-studio-service simulate [--scenario FILE]");
                return;
            } else {
                eprintln!("unknown simulator argument: {}", argument.to_string_lossy());
                process::exit(2);
            }
        }
        match env::current_exe() {
            Ok(executable) => {
                if let Err(error) = run_simulator(&executable, scenario.as_deref()) {
                    eprintln!("Studio simulator failed: {error}");
                    process::exit(1);
                }
            }
            Err(error) => {
                eprintln!("could not resolve simulator executable: {error}");
                process::exit(1);
            }
        }
        return;
    }
    let mut application_executable = None;
    let mut workspace_root = None;
    let mut active_profile_id = None;
    let mut fixture_mode = false;
    let mut stdio_mode = first.as_deref().is_some_and(|arg| arg == "--stdio");
    if first
        .as_deref()
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        println!(
            "vapor-studio-service --stdio --application-executable PATH [--workspace PATH] [--active-profile ID] [--fixture]"
        );
        return;
    }
    while let Some(arg) = args.next() {
        match arg.to_string_lossy().as_ref() {
            "--stdio" => stdio_mode = true,
            "--fixture" => fixture_mode = true,
            "--application-executable" => application_executable = args.next().map(PathBuf::from),
            "--workspace" => workspace_root = args.next().map(PathBuf::from),
            "--active-profile" => {
                active_profile_id = args.next().map(|id| id.to_string_lossy().into_owned())
            }
            "--help" | "-h" => {
                println!(
                    "vapor-studio-service --stdio --application-executable PATH [--workspace PATH] [--active-profile ID] [--fixture]"
                );
                return;
            }
            unknown => {
                eprintln!("unknown argument: {unknown}");
                process::exit(2);
            }
        }
    }
    if !stdio_mode {
        eprintln!(
            "private service mode required; start this executable through Vapor Studio or its simulator"
        );
        process::exit(2);
    }
    let Some(application_executable) = application_executable else {
        eprintln!("--application-executable is required");
        process::exit(2);
    };
    let bootstrap = ServiceBootstrap {
        application_executable,
        workspace_root,
        active_profile_id,
        fixture_mode,
    };
    if let Err(error) = run_stdio_service(bootstrap) {
        eprintln!("Vapor Studio service stopped: {error}");
        process::exit(1);
    }
}
