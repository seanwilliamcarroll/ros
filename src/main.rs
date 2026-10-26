use ovmf_prebuilt::{Arch, FileType, Prebuilt, Source};
use std::env;
use std::process::{Command, exit};

fn main() {
    let uefi = match env::args().nth(1).as_deref() {
        Some("uefi") => true,
        Some("bios") | None => false,
        Some(other) => {
            eprintln!("usage: cargo run [bios|uefi]  (got {other:?})");
            exit(1);
        }
    };

    let mut cmd = Command::new("qemu-system-x86_64");
    cmd.args(["-serial", "mon:stdio"]);
    cmd.args(["-device", "isa-debug-exit,iobase=0xf4,iosize=0x04"]);

    if uefi {
        let ovmf = Prebuilt::fetch(Source::LATEST, "target/ovmf").expect("fetch OVMF");
        let code = ovmf.get_file(Arch::X64, FileType::Code);
        let vars = ovmf.get_file(Arch::X64, FileType::Vars);
        cmd.arg("-drive")
            .arg(format!("format=raw,file={}", env!("UEFI_PATH")));
        cmd.arg("-drive").arg(format!(
            "if=pflash,format=raw,unit=0,file={},readonly=on",
            code.display()
        ));
        cmd.arg("-drive").arg(format!(
            "if=pflash,format=raw,unit=1,file={},snapshot=on",
            vars.display()
        ));
    } else {
        cmd.arg("-drive")
            .arg(format!("format=raw,file={}", env!("BIOS_PATH")));
    }

    let status = cmd.status().expect("failed to run qemu-system-x86_64");
    exit(match status.code() {
        Some(0x10) => 0, // isa-debug-exit success, (0x10 << 1) | 1 = 0x21 on the wire
        Some(0x11) => 1, // isa-debug-exit failure
        Some(c) => c,
        None => 2,
    });
}
