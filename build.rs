use std::path::PathBuf;

fn main() {
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let kernel = PathBuf::from(std::env::var_os("CARGO_BIN_FILE_KERNEL_kernel").unwrap());

    let mut boot_config = bootloader::BootConfig::default();
    boot_config.frame_buffer_logging = false; // keep the screen for your kernel
    boot_config.serial_logging = true; // still handy in the terminal

    let bios_path = out_dir.join("bios.img");
    bootloader::BiosBoot::new(&kernel)
        .set_boot_config(&boot_config)
        .create_disk_image(&bios_path)
        .unwrap();

    let uefi_path = out_dir.join("uefi.img");
    bootloader::UefiBoot::new(&kernel)
        .set_boot_config(&boot_config)
        .create_disk_image(&uefi_path)
        .unwrap();

    println!("cargo:rustc-env=BIOS_PATH={}", bios_path.display());
    println!("cargo:rustc-env=UEFI_PATH={}", uefi_path.display());
}
