/// Initializes a littlefs file system.
///
/// A partition with name `LITTLEFS_PARTITION_NAME` has to be specified
/// in the partition table csv file.
pub fn init_littlefs_storage() -> anyhow::Result<esp_idf_sys::esp_vfs_littlefs_conf_t> {
    use cstr::cstr;
    use esp_idf_sys::esp;

    let mut fs_conf = esp_idf_sys::esp_vfs_littlefs_conf_t {
        base_path: cstr!("/littlefs").as_ptr(),
        partition_label: cstr!("littlefs").as_ptr(),
        ..Default::default()
    };
    fs_conf.set_format_if_mount_failed(true as u8);
    fs_conf.set_dont_mount(false as u8);

    unsafe { esp!(esp_idf_sys::esp_vfs_littlefs_register(&fs_conf))? };
    let (mut fs_total_bytes, mut fs_used_bytes) = (0, 0);
    unsafe {
        esp!(esp_idf_sys::esp_littlefs_info(
            fs_conf.partition_label,
            &mut fs_total_bytes,
            &mut fs_used_bytes
        ))?
    };
    log::info!(
        "LittleFs Info: total bytes = {}, used bytes = {}.",
        fs_total_bytes,
        fs_used_bytes
    );

    Ok(fs_conf)
}
