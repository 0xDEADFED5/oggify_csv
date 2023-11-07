extern crate env_logger;
extern crate librespot_audio;
extern crate librespot_core;
extern crate librespot_metadata;
#[macro_use]
extern crate log;
extern crate csv;
extern crate regex;
extern crate scoped_threadpool;
extern crate tokio_core;

use chrono::Datelike;
use dateparser::parse;
use env_logger::{Builder, Env};
use librespot_audio::{AudioDecrypt, AudioFile};
use librespot_core::authentication::Credentials;
use librespot_core::config::SessionConfig;
use librespot_core::session::Session;
use librespot_core::spotify_id::SpotifyId;
use librespot_metadata::{FileFormat, Metadata, Track};
use m3u::EntryExt;
use regex::Regex;
use scoped_threadpool::Pool;
use std::error::Error;
use std::fs::{create_dir, DirEntry, File};
use std::io::Read;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use std::{env, fs};
use tokio_core::reactor::Core;
const MAX_LEN: usize = 140;

fn sanitize(input: &String) -> String {
    let mut result = input.clone();
    let ban: &str = r#":/\<>|?*"#;
    for c in ban.chars() {
        result = result.replace(c, "");
    }
    result = result.replace("\"", "'");
    result = result.replace("#", "%23");
    result = result.replace(" ", "_");
    result
}

fn scan_subfolder(path: &PathBuf) -> std::result::Result<Vec<DirEntry>, Box<dyn Error>> {
    let mut entries: Vec<DirEntry> = vec![];
    for e in fs::read_dir(path)? {
        entries.push(e?);
    }
    Ok(entries)
}

// get all filenames from a folder and it's subfolders
fn scan_folder(path: &PathBuf) -> std::result::Result<Vec<String>, Box<dyn Error>> {
    let mut results: Vec<PathBuf> = vec![];
    let mut meta;
    let mut entries = scan_subfolder(path)?;
    let mut subfolder_entries;
    let mut e;
    loop {
        if entries.is_empty() {
            break;
        }
        e = entries.pop().unwrap();
        meta = e.metadata()?;
        if meta.is_file() {
            results.push(e.path());
        } else if meta.is_dir() {
            subfolder_entries = scan_subfolder(&e.path())?;
            entries.append(&mut subfolder_entries);
        }
    }
    Ok(results
        .iter()
        .map(|r| r.to_string_lossy().to_string())
        .collect())
}
fn main() {
    Builder::from_env(Env::default().default_filter_or("info")).init();
    let args: Vec<_> = env::args().collect();
    let mut dir;
    match args.len() {
        3 => {
            dir = env::current_exe().unwrap();
            dir.pop();
        }
        4 => {
            dir = PathBuf::from(&args[3]);
        }
        _ => {
            println!("Usage: {} user password path(optional)", args[0]);
            println!("Press any key to continue...");
            let _ = std::io::Read::read(&mut std::io::stdin(), &mut [0u8]).unwrap();
            return;
        }
    }
    let mut core = Core::new().unwrap();
    let handle = core.handle();
    let session_config = SessionConfig::default();
    let credentials = Credentials::with_password(args[1].to_owned(), args[2].to_owned());
    info!("connecting ...");
    let session = core
        .run(Session::connect(session_config, credentials, None, handle))
        .unwrap();
    info!("connected!");

    let mut threadpool = Pool::new(1);
    let spotify_uri = Regex::new(r"spotify:track:([[:alnum:]]+)").unwrap();
    //let spotify_url = Regex::new(r"open\.spotify\.com/track/([[:alnum:]]+)").unwrap();
    let files: Vec<_> = scan_folder(&dir)
        .unwrap()
        .into_iter()
        .filter(|f| f.ends_with(".csv"))
        .collect();
    info!("{} CSV files found", files.len());
    let mut rdr;
    for f in files {
        let s = sanitize(&f[0..f.len() - 4].to_string());
        let mut path = PathBuf::from(&s);
        let m3u_path = PathBuf::from(s + ".m3u");

        if m3u_path.exists() {
            info! {"'{}' already exists, deleting...", &m3u_path.display()};
            if let Err(e) = fs::remove_file(&m3u_path) {
                error!("error deleting '{}' : {}", &m3u_path.display(), e);
                break;
            }
        }
        let mut m3u = File::create(&m3u_path).unwrap();
        let mut m3u_writer = m3u::Writer::new_ext(&mut m3u).unwrap();
        if let Err(e) = create_dir(path.clone()) {
            warn!("error creating folder '{}': {}", &path.display(), e);
        } else {
            info!("folder created '{}'", &path.display());
        }
        info!("reading '{}'...", f);
        rdr = csv::Reader::from_path(f).unwrap();
        let mut entries: Vec<EntryExt> = vec![];
        for r in rdr.records() {
            let r = r.unwrap();
            let year;
            if r[8].len() == 4 {
                // -_-
                year = *(&r[8].parse::<i32>().unwrap());
            } else {
                year = parse(&r[8]).unwrap().year();
            }
            let duration = *(&r[12].parse::<u32>().unwrap()) as f64 / 1000.0;
            // artist - album (year) - disc - track - track name
            let mut filename = format!(
                "{}-{}({})-D{:0>2}-T{:0>2}-{}.ogg",
                &r[3], &r[5], year, &r[10], &r[11], &r[1]
            );
            filename = filename.replace(", ", ",");
            if filename.len() + path.to_string_lossy().len() + 1 > MAX_LEN {
                filename = format!("{}-{}.ogg", &r[3], &r[1]);
            }
            filename = sanitize(&filename);
            let rel_path = format!(
                "{}/{}",
                path.file_stem().unwrap().to_string_lossy(),
                filename
            );
            // don't download existing files
            path.push(&filename);
            if path.exists() {
                info!("file exists '{}', skipping...", &path.display());
                entries.push(
                    m3u::path_entry(&rel_path).extend(duration, format!("{} - {}", &r[3], &r[1])),
                );
                path.pop();
                continue;
            }
            let id = spotify_uri
                .captures(&r[0])
                .or_else(|| {
                    warn!("cannot parse track from string: '{}'", &r[0]);
                    None
                })
                .and_then(|capture| SpotifyId::from_base62(&capture[1]).ok())
                .unwrap();
            info!("getting track '{}' ...", id.to_base62());
            let mut track = core
                .run(Track::get(&session, id))
                .expect("cannot get track metadata");
            if !track.available {
                warn!(
                    "track '{}' is not available, finding alternative...",
                    id.to_base62()
                );
                let alt_track = track.alternatives.iter().find_map(|id| {
                    let alt_track = core
                        .run(Track::get(&session, *id))
                        .expect("cannot get track metadata");
                    match alt_track.available {
                        true => Some(alt_track),
                        false => None,
                    }
                });
                if alt_track.is_none() {
                    error!("could not find alternative for track '{}'", id.to_base62());
                    error!(
                        "missing track: '{}' by '{}' from '{}' ({})",
                        &r[1], &r[3], &r[5], year
                    );
                    path.pop();
                    continue;
                }
                track = alt_track.unwrap();
                warn!(
                    "found track alternative '{}' -> '{}'",
                    id.to_base62(),
                    track.id.to_base62()
                );
            }
            // could totally crash here, didn't for me yet.
            let file_id = track
                .files
                .get(&FileFormat::OGG_VORBIS_320)
                .or(track.files.get(&FileFormat::OGG_VORBIS_160))
                .or(track.files.get(&FileFormat::OGG_VORBIS_96))
                .expect("could not find a OGG_VORBIS format for the track.");
            let key = core
                .run(session.audio_key().request(track.id, *file_id))
                .expect("cannot get audio key");
            let mut encrypted_file = core.run(AudioFile::open(&session, *file_id)).unwrap();
            let mut buffer = Vec::new();
            let mut read_all: std::io::Result<usize> = Ok(0);
            let fetched = AtomicBool::new(false);
            threadpool.scoped(|scope| {
                scope.execute(|| {
                    read_all = encrypted_file.read_to_end(&mut buffer);
                    fetched.store(true, Ordering::Release);
                });
                while !fetched.load(Ordering::Acquire) {
                    core.turn(Some(Duration::from_millis(100)));
                }
            });
            read_all.expect("cannot read file stream");
            let mut decrypted_buffer = Vec::new();
            AudioDecrypt::new(key, &buffer[..])
                .read_to_end(&mut decrypted_buffer)
                .expect("cannot decrypt stream");
            std::fs::write(&path, &decrypted_buffer[0xa7..])
                .expect(format!("cannot write decrypted track to '{}'", &path.display()).as_str());
            info!("track downloaded: '{}'", &path.display());
            entries.push(
                m3u::path_entry(&rel_path).extend(duration, format!("{} - {}", &r[3], &r[1])),
            );
            path.pop();
        }
        for e in &entries {
            m3u_writer.write_entry(e).unwrap();
        }
        m3u_writer.flush().unwrap();
        info!("M3U '{}' finished.", &m3u_path.display());
    }
    println!("Press any key to continue...");
    let _ = std::io::Read::read(&mut std::io::stdin(), &mut [0u8]).unwrap();
    return;
}
