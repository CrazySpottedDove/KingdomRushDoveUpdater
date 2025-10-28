use rayon::prelude::*;
use reqwest::blocking::Client;
use serde_json::Value;
use std::fs;
use std::io::{self, Read};
const LOCAL_COMMIT_FILE: &str = "./current_version_commit_hash.txt";
const REPO_MASTER_COMMIT_HASH_API: &str = "https://dgithub.xyz/CrazySpottedDove/KingdomRushDove/commits/deferred_commit_data/master?original_branch=master";
const BASE_DOWNLOAD_URL: &str = "https://dgithub.xyz/CrazySpottedDove/KingdomRushDove/raw/master/";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const RED: &str = "\x1b[31m";
const CYAN: &str = "\x1b[36m";
const RESET: &str = "\x1b[0m";
use std::path::Path;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Step 1: 读取本地 commit-hash
    let local_commit_hash = read_local_commit_hash()?;
    println!("{CYAN}本地版本: {local_commit_hash}{RESET}");

    // Step 2: 获取远程仓库的最新 commit-hash
    let remote_commit_hash = fetch_remote_commit_hash()?;
    println!("{CYAN}最新版本: {remote_commit_hash}{RESET}");

    // Step 3: 比较 commit-hash
    if local_commit_hash == remote_commit_hash {
        println!("{GREEN}已是最新，无需更新。{RESET}");
        println!("{CYAN}按回车键退出...{RESET}");
        let mut _wait = String::new();
        std::io::stdin().read_line(&mut _wait).ok();
        return Ok(());
    }

    println!("{YELLOW}检测到新版本，准备更新...{RESET}");

    // Step 4: 获取差分文件列表
    let diff_files = fetch_diff_files(&local_commit_hash, &remote_commit_hash)?;
    println!("{CYAN}需更新代码文件: {:?}{RESET}", diff_files);

    // TODO: 下载差分文件并更新本地文件
    diff_files
        .par_iter()
        .map(|file| download_and_replace_file(file).map_err(|e| format!("{}: {}", file, e)))
        .collect::<Result<Vec<_>, _>>()?;

    update_assets()?;
    println!("{GREEN}全部更新完成！{RESET}");

    // 写回最新 commit_hash
    fs::write(LOCAL_COMMIT_FILE, &remote_commit_hash)?;
    println!("{GREEN}已更新本地版本记录。{RESET}");

    println!("{CYAN}按回车键退出...{RESET}");
    let mut _wait = String::new();
    std::io::stdin().read_line(&mut _wait).ok();

    Ok(())
}

fn read_local_commit_hash() -> io::Result<String> {
    let mut file = fs::File::open(LOCAL_COMMIT_FILE)?;
    let mut hash = String::new();
    file.read_to_string(&mut hash)?;
    Ok(hash.trim().to_string())
}

fn fetch_remote_commit_hash() -> Result<String, Box<dyn std::error::Error>> {
    let client = Client::new();
    let response = client
        .get(REPO_MASTER_COMMIT_HASH_API)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36 Edg/141.0.0.0")
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .header("X-Requested-With", "XMLHttpRequest")
        .send()?;

    if !response.status().is_success() {
        return Err(format!(
            "Failed to fetch remote commit hash. HTTP Status: {}",
            response.status()
        )
        .into());
    }
    let response_text = response.text()?;
    // 打印响应内容以调试
    // println!("Remote commit API response: {}", response_text);

    let json: Value = serde_json::from_str(&response_text)?;
    let deferred_commits = json["deferredCommits"]
        .as_array()
        .ok_or("Failed to parse 'deferredCommits' array")?;

    // 获取第一个 commit 的 oid
    let remote_commit_hash = deferred_commits
        .get(0)
        .and_then(|commit| commit["oid"].as_str())
        .ok_or("Failed to parse remote commit hash")?;

    Ok(remote_commit_hash.to_string())
}
use scraper::{Html, Selector};

fn fetch_diff_files(
    local_commit: &str,
    remote_commit: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    // 构造差异文件的 URL
    let diff_url = format!(
        "https://dgithub.xyz/CrazySpottedDove/KingdomRushDove/compare/file-list?range={}...{}",
        local_commit, remote_commit
    );

    let client = Client::new();
    let response = client
        .get(&diff_url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36 Edg/141.0.0.0")
        .send()?
        .text()?;

    // 使用 scraper 解析 HTML
    let document = Html::parse_document(&response);

    // 匹配文件名的 <a> 标签
    let file_selector = Selector::parse("ol.content li a").unwrap();

    // 提取文件名并清理多余的空白字符
    let files: Vec<String> = document
        .select(&file_selector)
        .filter_map(|element| {
            element
                .text()
                .next() // 获取 <a> 标签的文本内容
                .map(|text| text.trim().to_string()) // 去除前后空白字符
        })
        .filter(|file| !file.is_empty()) // 过滤掉空字符串
        .collect();

    if files.is_empty() {
        return Err("No files found in the diff response.".into());
    }

    Ok(files)
}

fn download_and_replace_file(file: &str) -> Result<(), Box<dyn std::error::Error>> {
    let url = format!("{}{}", BASE_DOWNLOAD_URL, file);
    let client = Client::new();
    let response = client.get(&url).send()?;

    if !response.status().is_success() {
        eprintln!(
            "{RED}下载失败: {} 状态码: {}{RESET}",
            file,
            response.status()
        );
        return Err(format!(
            "Failed to download file: {}. HTTP Status: {}",
            file,
            response.status()
        )
        .into());
    }

    let content = response.bytes()?;
    let path = Path::new(file);

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(path, &content)?;
    println!("{GREEN}已更新: {}{RESET}", file);

    Ok(())
}
use std::collections::HashMap;
fn update_assets() -> Result<(), Box<dyn std::error::Error>> {
    println!("{CYAN}检查资源文件...{RESET}");
    // Step 1: 读取 `assets_index.lua`
    let assets_index = read_assets_index("_assets/assets_index.lua")?;
    let assets_dir = "_assets";
    let trashed_dir = "_trashed_assets";

    // Step 2: 检查本地资源文件
    let mut download_batches: HashMap<String, Vec<String>> = HashMap::new();
    for (path, info) in &assets_index {
        let fullpath = format!("{}/{}", assets_dir, path);
        let local_size = file_size(&fullpath);
        let filename = Path::new(&path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&path);
        if local_size != *info {
            println!(
                "{YELLOW}资源缺失或过期: {} (本地: {}, 需: {}){RESET}",
                path, local_size, info
            );
            let release = get_release_for_file(filename);
            download_batches
                .entry(release)
                .or_insert_with(Vec::new)
                .push(path.clone());
        }
    }

    // Step 3: 下载缺失或过期的资源
    // let client = Client::new();
    for (release, files) in download_batches {
        use regex::Regex;
        // ...existing code...

        files.par_iter().for_each(|file| {
            let filename = Path::new(&file)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&file);

            let re_square = Regex::new(r"\[([^\]]+)\]").unwrap();
            let replaced =
                re_square.replace_all(filename, |caps: &regex::Captures| format!(".{}.", &caps[1]));
            let re_round = Regex::new(r"\(([^)]+)\)").unwrap();
            let replaced = re_round.replace_all(&replaced, |caps: &regex::Captures| {
                format!(".{}.", &caps[1])
            });
            let re_dot = Regex::new(r"\.+").unwrap();
            let url_filename = re_dot.replace_all(&replaced, ".");

            let url = format!(
                "https://dgithub.xyz/CrazySpottedDove/KingdomRushDove/releases/download/{}/{}",
                release, url_filename
            );
            println!("{CYAN}下载: {}{RESET}", url);

            // 每个线程独立创建 client，避免并发冲突
            use std::time::Duration;
            let client = Client::builder()
                .timeout(Duration::from_secs(300)) // 5分钟
                .build()
                .unwrap();
            match client.get(&url).send() {
                Ok(response) if response.status().is_success() => match response.bytes() {
                    Ok(content) => {
                        let fullpath = format!("{}/{}", assets_dir, file);
                        if let Some(parent) = Path::new(&fullpath).parent() {
                            let _ = fs::create_dir_all(parent);
                        }
                        if fs::write(&fullpath, &content).is_ok() {
                            println!("{GREEN}资源已下载: {}{RESET}", file);
                        }
                    }
                    Err(e) => eprintln!("{RED}写入失败: {}: {:?}{RESET}", file, e),
                },
                Ok(response) => eprintln!(
                    "{RED}下载失败: {} 状态码: {}{RESET}",
                    file,
                    response.status()
                ),
                Err(e) => eprintln!("{RED}请求失败: {}: {:?}{RESET}", file, e),
            }
        });
    }

    // Step 4: 清理多余或不匹配的资源
    trash_unindexed_assets(&assets_index, &assets_dir, &trashed_dir)?;

    println!("{GREEN}资源检查完成。{RESET}");
    Ok(())
}

use mlua::Lua;

fn read_assets_index(path: &str) -> Result<HashMap<String, u64>, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    let lua = Lua::new();
    let table: mlua::Table = lua.load(&content).eval()?;

    let mut index = HashMap::new();
    for pair in table.pairs::<String, mlua::Table>() {
        let (key, value_table) = pair?;
        let size: u64 = value_table.get("size")?;
        index.insert(key, size);
    }
    Ok(index)
}

fn file_size(path: &str) -> u64 {
    fs::metadata(path).map(|meta| meta.len()).unwrap_or(0)
}

fn get_release_for_file(filename: &str) -> String {
    // 去除扩展名（最后一个点及其后内容）
    let name = match filename.rfind('.') {
        Some(idx) => &filename[..idx],
        None => filename,
    };
    let len = name.chars().count();
    if len == 0 {
        return "other".to_string();
    }
    // Lua: local mid = math.floor((len + 1) / 2)
    let mid = (len + 1) / 2 - 1; // Rust 0-based
    let ch = name.chars().nth(mid).unwrap_or('o').to_ascii_lowercase();
    if ch.is_ascii_alphanumeric() {
        ch.to_string()
    } else {
        "other".to_string()
    }
}

fn trash_unindexed_assets(
    index: &HashMap<String, u64>,
    assets_dir: &str,
    trashed_dir: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    for entry in fs::read_dir(assets_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let relpath = path
                .strip_prefix(assets_dir)?
                .to_str()
                .unwrap_or("")
                .to_string();
            if relpath != "assets_index.lua" && !index.contains_key(&relpath) {
                let trash_path = format!("{}/{}", trashed_dir, relpath);
                if let Some(parent) = Path::new(&trash_path).parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::rename(&path, &trash_path)?;
                println!("{YELLOW}多余文件已移至回收站: {}{RESET}", trash_path);
            }
        }
    }
    Ok(())
}
