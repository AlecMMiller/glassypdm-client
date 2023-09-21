// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use merkle_hash::camino::Utf8PathBuf;
use serde::{Serialize, Deserialize};
use tauri::api::dialog;
use tauri::{CustomMenuItem, Menu, MenuItem, Submenu, Manager};
use merkle_hash::{bytes_to_hex, Algorithm, MerkleTree, anyhow::Error};
use std::fs::{File, self};
use std::io::prelude::*;
use std::io::Read;
use reqwest::multipart::Part;
use tauri::api::path::*;
use tauri::PathResolver;

#[derive(Serialize, Deserialize)]
struct CADFile {
    path: String,
    size: u64,
    hash: String,
    is_file: bool
}

fn get_file_as_byte_vec(filename: &String) -> Vec<u8> {
    let mut f = File::open(&filename).expect("no file found");
    let metadata = fs::metadata(&filename).expect("unable to read metadata");
    let mut buffer = vec![0; metadata.len() as usize];
    f.read(&mut buffer).expect("buffer overflow");

    buffer
}

#[tauri::command]
async fn upload_changes(app_handle: tauri::AppHandle, files: Vec<CADFile>, commit: u64) -> Result<(), ()> {
    let client = reqwest::Client::new();
    println!("commit # {}", commit);

    // TODO: need to subtract project dir from file path
    let project_dir = get_project_dir(app_handle);
    println!("{}", project_dir);

    for file in files {
        let path: String = file.path;
        let relative_path = path.replace(&project_dir, "");

        println!("uploading {}", path);
        println!("relative {}", relative_path);
        let content: Vec<u8> = get_file_as_byte_vec(&path);
        let part = Part::bytes(content).file_name(path.clone());
        let request = reqwest::multipart::Form::new()
            .part("key", part)
            .text("commit", commit.to_string())
            .text("path", relative_path)
            .text("size", file.size.to_string())
            .text("hash", file.hash);

        let _response = client.post("http://localhost:5000/ingest")
            .multipart(request)
            .send().await;

    }
    println!("upload done");
    Ok(())
}

#[tauri::command]
fn get_project_dir(app_handle: tauri::AppHandle) -> String {
    let appdir = app_handle.path_resolver().app_local_data_dir().unwrap();
    let path = appdir.join("project_dir.txt");
    println!("{}", path.as_path().display());
    let output = fs::read_to_string(path).expect("no lol");
    return output;
}

fn update_project_dir(app_handle: tauri::AppHandle, dir: PathBuf) {
    println!("new project dir: {}", dir.display());

    let appdir = app_handle.path_resolver().app_local_data_dir().unwrap();
    let mut path = appdir.join("project_dir.txt");

    let _ = fs::write(path, pathbuf_to_string(dir));

    path = appdir.join("base.json");
    get_changes(app_handle, &pathbuf_to_string(path));
}

fn pathbuf_to_string(path: PathBuf) -> String {
    let output: String = path.into_os_string().into_string().unwrap();
    return output;
}

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
fn greet(name: &str) -> String {
    let output: String = format!("Hello, {}! You've been greeted from Rust!", name);
    println!("helasdfasdfasdfsdf");
    return output;
}

#[tauri::command]
fn get_changes(app_handle: tauri::AppHandle, results_path: &str) {
    let path: String = get_project_dir(app_handle);
    if path == "no lol" {
        return;
    }
    
    let mut files: Vec<CADFile> = Vec::new();

    let do_steps = || -> Result<(), Error> {
        let tree = MerkleTree::builder(path)
        .algorithm(Algorithm::Blake3)
        .hash_names(true)
        .build()?;

        for item in tree {
            let pathbuf = item.path.absolute.into_string();
            let s_hash = bytes_to_hex(item.hash);

            println!("{}: {}", pathbuf, s_hash);
            if pathbuf.as_str() == "" {
                continue;
            }
            let metadata = std::fs::metadata(pathbuf.as_str())?;
            let isthisfile = metadata.is_file();
            let filesize = metadata.len();

            if !isthisfile {
                continue;
            }
            
            let file = CADFile {
                hash: s_hash,
                path: pathbuf,
                size: filesize,
                is_file: isthisfile
            };
            files.push(file);
        }

        let json = serde_json::to_string(&files)?;

        let mut file = File::create(results_path)?;
        file.write_all(json.as_bytes())?;
        Ok(())
    };
    let _ = do_steps();

    println!("fn get_changes done");
}

fn main() {
    let set_dir = CustomMenuItem::new("Set Project Directory".to_string(), "Set Project Directory");
    let file_menu = Submenu::new("Configure", Menu::new().add_item(set_dir));
    let menu = Menu::new()
      .add_submenu(file_menu);

    tauri::Builder::default()
        .menu(menu)
        .on_menu_event(|event| match event.menu_item_id() {
        "Set Project Directory" => {
            dialog::FileDialogBuilder::default()
            .pick_folder(move |path_buf| match path_buf {
                Some(p) => {
                    println!("{}", p.display());
                    update_project_dir(event.window().app_handle(), p);
                }
                _ => {}
            });
        }
        _ => {}
        })
        .invoke_handler(tauri::generate_handler![get_changes, greet, get_project_dir, upload_changes])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
