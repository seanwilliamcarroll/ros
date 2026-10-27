use std::env;
use std::path::PathBuf;
use std::process::{Command, exit};
use std::thread::sleep;
use std::time::{Duration, Instant};

const TEST_TIMEOUT: Duration = Duration::from_secs(300);

/// isa-debug-exit makes QEMU exit with (value << 1) | 1.
const QEMU_SUCCESS: i32 = (0x10 << 1) | 1; // 33
const QEMU_FAILURE: i32 = (0x11 << 1) | 1; // 35

fn has_cargo_hash_suffix(path: &std::path::Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .and_then(|n| n.rsplit_once('-'))
        .is_some_and(|(_, hash)| hash.len() == 16 && hash.bytes().all(|b| b.is_ascii_hexdigit()))
}

fn main() {
    let mut args = env::args().skip(1);
    let kernel = PathBuf::from(args.next().expect("usage: ros-runner <kernel-elf> [uefi]"));
    let rest: Vec<String> = args.collect(); // harness args from `cargo test`; ignored

    // Same heuristic bootimage used: `cargo test` binaries live in .../deps/
    let is_test = match env::var("ROS_RUNNER_MODE").as_deref() {
        Ok("test") => true,
        Ok("run") => false,
        _ => has_cargo_hash_suffix(&kernel),
    };
    let uefi = rest.iter().any(|a| a == "uefi");

    // Keep test output clean; keep serial logs for normal runs (useful for boot debugging).
    let mut boot_config = bootloader::BootConfig::default();
    boot_config.frame_buffer_logging = false;
    boot_config.serial_logging = !is_test;

    let image = kernel.with_extension(if uefi { "uefi.img" } else { "bios.img" });
    if uefi {
        bootloader::UefiBoot::new(&kernel)
            .set_boot_config(&boot_config)
            .create_disk_image(&image)
            .expect("failed to create UEFI image");
    } else {
        bootloader::BiosBoot::new(&kernel)
            .set_boot_config(&boot_config)
            .create_disk_image(&image)
            .expect("failed to create BIOS image");
    }

    let mut cmd = Command::new("qemu-system-x86_64");
    cmd.arg("-drive")
        .arg(format!("format=raw,file={}", image.display()));
    cmd.args(["-serial", "mon:stdio"]);
    cmd.args(["-device", "isa-debug-exit,iobase=0xf4,iosize=0x04"]);
    if is_test {
        cmd.args(["-display", "none"]);
    }
    if uefi {
        // Anchor to the repo, not the cwd (which differs between `cargo run` and `cargo test`).
        let ovmf_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../target/ovmf");
        let ovmf = ovmf_prebuilt::Prebuilt::fetch(ovmf_prebuilt::Source::LATEST, ovmf_dir)
            .expect("failed to fetch OVMF");
        let code = ovmf.get_file(ovmf_prebuilt::Arch::X64, ovmf_prebuilt::FileType::Code);
        let vars = ovmf.get_file(ovmf_prebuilt::Arch::X64, ovmf_prebuilt::FileType::Vars);
        cmd.arg("-drive").arg(format!(
            "if=pflash,format=raw,unit=0,file={},readonly=on",
            code.display()
        ));
        cmd.arg("-drive").arg(format!(
            "if=pflash,format=raw,unit=1,file={},snapshot=on",
            vars.display()
        ));
    }

    let mut child = cmd.spawn().expect("failed to start qemu-system-x86_64");
    let deadline = Instant::now() + TEST_TIMEOUT;
    let status = loop {
        if let Some(status) = child.try_wait().expect("failed to wait on qemu") {
            break status;
        }
        if is_test && Instant::now() > deadline {
            let _ = child.kill();
            eprintln!("ros-runner: test timed out after {TEST_TIMEOUT:?}");
            exit(2);
        }
        sleep(Duration::from_millis(50));
    };

    let code = status.code();
    exit(match (is_test, code) {
        (_, Some(QEMU_SUCCESS)) => 0,
        (_, Some(QEMU_FAILURE)) => 1,
        (false, Some(0)) => 0, // you closed the QEMU window
        (true, _) => 1,        // a test that exits any other way failed
        (false, Some(c)) => c,
        (false, None) => 2,
    });
}
