use std::{
    fs::{create_dir_all, metadata, File},
    io::{copy, Write},
    path::Path,
};
use tokio::runtime::Runtime;
use zip::ZipArchive;

#[derive(Debug)]
pub enum DownloadError {
    Reqwest(reqwest::Error),
    IO(std::io::Error),
}

pub fn download_file(url: String, file_path: &str) -> Result<(), DownloadError> {
    use DownloadError::*;
    Runtime::new()
        .expect("Failed to create tokio runtime")
        .block_on(async {
            let response = reqwest::get(url).await.map_err(Reqwest)?;
            let bytes = response.bytes().await.map_err(DownloadError::Reqwest)?;
            let mut file = create_file(file_path).map_err(IO)?;
            file.write_all(&bytes).map_err(IO)?;
            Ok(())
        })
}

pub fn create_file(file_path: &str) -> Result<File, std::io::Error> {
    if let Some(parent_dir) = Path::new(&file_path).parent() {
        create_dir_all(parent_dir)?;
    }
    File::create(file_path)
}

#[cfg(unix)]
pub fn set_exe_permission(file_path: &String) -> Result<(), std::io::Error> {
    use std::os::unix::fs::PermissionsExt;
    let file = std::fs::OpenOptions::new().write(true).open(file_path)?;
    let mut permissions = file.metadata()?.permissions();
    permissions.set_mode(permissions.mode() | 0o100);
    std::fs::set_permissions(file_path, permissions)
}
#[cfg(not(unix))]
pub fn set_exe_permission(_: &String) -> Result<(), std::io::Error> {
    println!("Can't set exe permission on non unix platform. It probably is already set");
    Ok(())
}

pub fn unzip(zip_path: &str, dir: &str) -> Result<(), std::io::Error> {
    let mut archive = ZipArchive::new(File::open(zip_path)?)?;
    create_dir_all(dir)?;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let path = format!("{}/{}", dir, file.mangled_name().display());
        copy(&mut file, &mut create_file(&path)?)?;
    }
    Ok(())
}

pub fn ensure<D>(name: String, path: &String, download: D)
where
    D: Fn(&String) -> Result<(), DownloadError>,
{
    match metadata(path).or_else(|_| {
        download(path)?;
        metadata(path).map_err(DownloadError::IO)
    }) {
        Err(download_error) => println!("{:?}", download_error),
        _ => println!("{name} found"),
    }
}
