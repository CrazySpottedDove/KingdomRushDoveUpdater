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
const WORK_DIR: &str = "Kingdom Rush";
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if !cfg!(debug_assertions) {
        if !is_current_dir_safe() {
            println!(
                "{RED}你在错误的目录运行了更新程序！请将更新程序放置在 {WORK_DIR} 目录后再运行。{RESET}"
            );
            wait_for_enter();
            return Ok(());
        }
    }

    // Step 1: 读取本地 commit-hash
    let local_commit_hash = read_local_commit_hash()?;
    // println!("{CYAN}本地版本: {local_commit_hash}{RESET}");

    // 异步拉取 Gitee 日志：在后台线程执行，不阻塞下载

    let owner = "CrazySpottedDove".to_string();
    let repo = "KingdomRushDove".to_string();
    let local = local_commit_hash.clone();
    // let remote = remote_commit_hash.clone();

    // 启动一个线程同时获取和打印日志
    let handle = std::thread::spawn(move || {
        match fetch_commit_logs_gitee(&owner, &repo, &local) {
            Ok(logs) => {
                if !logs.is_empty() {
                    println!("{CYAN}本次更新内容（来自 Gitee）：{RESET}");
                    for line in logs {
                        println!("{YELLOW}{}{RESET}", line);
                    }
                }
            }
            Err(e) => {
                // 拉取失败：打印错误信息（可选）
                eprintln!("{RED}拉取更新日志失败：{}{RESET}", e);
            }
        }
    });

    // Step 2: 获取远程仓库的最新 commit-hash
    println!("{CYAN}正在检查最新版本...{RESET}");
    let remote_commit_hash = fetch_remote_commit_hash()?;
    // println!("{CYAN}最新版本: {remote_commit_hash}{RESET}");

    // Step 3: 比较 commit-hash
    if local_commit_hash == remote_commit_hash {
        println!("{GREEN}已是最新，无需更新。{RESET}");
        println!("{YELLOW}如果想强行检查美术资源，请输入c并回车{RESET}");
        println!("{YELLOW}否则，按回车退出{RESET}");

        let mut input = String::new();
        std::io::stdin().read_line(&mut input).ok();
        if input.trim().eq_ignore_ascii_case("c") {
            if let Err(e) = update_assets() {
                eprintln!("{RED}资源检查/更新失败：{}{RESET}", e);
            } else {
                println!("{GREEN}美术资源检查/更新完成！{RESET}");
            }
            wait_for_enter();
        }
        return Ok(());
    }

    println!("{GREEN}检测到新版本，准备更新...{RESET}");

    // Step 4: 获取差分文件列表
    let diff_files = fetch_diff_files(&local_commit_hash, &remote_commit_hash)?;
    for diff_file in &diff_files {
        println!("{YELLOW}   {diff_file}{RESET}");
    }
    println!("{CYAN}正在下载新文件...{RESET}");

    // 下载差分文件并更新本地文件
    let results: Vec<_> = diff_files
        .par_iter()
        .map(|file| download_and_replace_file(file).map_err(|e| format!("{}: {}", file, e)))
        .collect();

    let errors: Vec<_> = results.into_iter().filter_map(|res| res.err()).collect();

    if !errors.is_empty() {
        println!("{RED}部分文件下载失败，未更新本地版本记录：{RESET}");
        for err in errors {
            println!("{RED}  - {}{RESET}", err);
        }
        println!("{CYAN}请修复网络或稍后重试。{RESET}");

        wait_for_enter();
        return Ok(());
    }

    let _ = handle.join();

    update_assets()?;

    println!("{GREEN}全部更新完成！{RESET}");

    // 写回最新 commit_hash
    fs::write(LOCAL_COMMIT_FILE, &remote_commit_hash)?;
    println!("{GREEN}已更新本地版本记录。{RESET}");
    wait_for_enter();
    Ok(())
}

fn is_current_dir_safe() -> bool {
    let current_dir = match std::env::current_dir() {
        Ok(dir) => dir,
        Err(_) => return false,
    };
    if let Some(dir_name) = current_dir.file_name() {
        if dir_name == WORK_DIR {
            return true;
        }
    }
    false
}

fn wait_for_enter() {
    println!("{CYAN}按回车键退出...{RESET}");
    let mut _wait = String::new();
    std::io::stdin().read_line(&mut _wait).ok();
}

fn read_local_commit_hash() -> io::Result<String> {
    let mut file = fs::File::open(LOCAL_COMMIT_FILE)?;
    let mut hash = String::new();
    file.read_to_string(&mut hash)?;
    Ok(hash.trim().to_string())
}

fn fetch_remote_commit_hash() -> Result<String, Box<dyn std::error::Error>> {
    let client = reqwest::blocking::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()?;
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

    // println!("{CYAN}Diff URL: {diff_url}{RESET}");

    let client = reqwest::blocking::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()?;
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
        .filter(|file| !file.is_empty() && file != "current_version_commit_hash.txt") // 过滤掉空字符串
        .collect();

    if files.is_empty() {
        return Err("No files found in the diff response.".into());
    }

    Ok(files)
}

fn download_and_replace_file(file: &str) -> Result<(), Box<dyn std::error::Error>> {
    let url = format!("{}{}", BASE_DOWNLOAD_URL, file);
    let client = reqwest::blocking::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()?;
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
    // println!("{GREEN}已更新: {}{RESET}", file);

    Ok(())
}

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::collections::HashMap;
use std::io::Write;
use std::sync::{Arc, Mutex};
const PROXY_LIST: [&str; 3] = [
    "https://hub.gitmirror.com/https://github.com",
    "https://dgithub.xyz",
    "https://bgithub.xyz",
];
const MAX_RETRY: u64 = 3;
const SPEED_CHECK_INTERVAL: std::time::Duration = std::time::Duration::from_secs(2);
const MIN_SPEED: u64 = 10 * 1024; // 10KB/s

fn update_assets() -> Result<(), Box<dyn std::error::Error>> {
    let assets_index = read_assets_index("_assets/assets_index.lua")?;
    let assets_dir = "_assets";
    let trashed_dir = "_trashed_assets";

    let mut download_batches: HashMap<String, Vec<(String, u64)>> = HashMap::new();
    let mut assets_count = 0;
    for (path, info) in &assets_index {
        let fullpath = format!("{}/{}", assets_dir, path);
        let local_size = file_size(&fullpath);
        let filename = Path::new(&path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&path);
        if local_size != *info {
            let release = get_release_for_file(filename);
            download_batches
                .entry(release)
                .or_insert_with(Vec::new)
                .push((path.clone(), *info));
            assets_count += 1;
        }
    }

    println!(
        "{CYAN}需要下载或更新的美术资源数量: {} 个{RESET}",
        assets_count
    );

    let m = Arc::new(MultiProgress::new());
    let failed_files = Arc::new(Mutex::new(Vec::new()));

    let mut handles = vec![];

    for (release, files) in download_batches {
        let m = Arc::clone(&m);
        let failed_files = Arc::clone(&failed_files);
        let assets_dir = assets_dir.to_string();

        let handle = std::thread::spawn(move || {
            use regex::Regex;
            files.par_iter().for_each(|(file, file_size)| {
                let filename = Path::new(&file)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&file);

                let re_square = Regex::new(r"\[([^\]]+)\]").unwrap();
                let replaced = re_square
                    .replace_all(filename, |caps: &regex::Captures| format!(".{}.", &caps[1]));
                let re_round = Regex::new(r"\(([^)]+)\)").unwrap();
                let replaced = re_round.replace_all(&replaced, |caps: &regex::Captures| {
                    format!(".{}.", &caps[1])
                });
                let replaced = replaced.replace("'", ".");
                let replaced = replaced.replace(" ", ".");
                let re_dot = Regex::new(r"\.+").unwrap();
                let url_filename = re_dot.replace_all(&replaced, ".");

                use std::time::Duration;

                let pb = m.add(
                    ProgressBar::new(*file_size)
                        .with_style(
                            ProgressStyle::default_bar()
                                .template("{spinner:.green} {msg} [{bar:40.cyan/blue}] {bytes}/{total_bytes} {bytes_per_sec} {percent}%")
                                .unwrap()
                                .progress_chars("==-"),
                        )
                );
                pb.set_message(format!("下载中: {}", filename));
                let mut success = false;
                for retry in 0..MAX_RETRY {
                    let proxy = PROXY_LIST[(retry as usize) % PROXY_LIST.len()];

                    if retry > 0 {
                        pb.reset();
                        pb.set_position(0);
                        pb.set_message(format!("使用镜像{}重试中({}/{}) {}",proxy, retry + 1, MAX_RETRY, filename));
                    }
                    let url = format!("{proxy}/CrazySpottedDove/KingdomRushDove/releases/download/{release}/{url_filename}");
                    let client = Client::builder()
                        .danger_accept_invalid_certs(true)
                        .timeout(Duration::from_secs(60))
                        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36 Edg/141.0.0.0")
                        .build()
                        .unwrap();
                    match client.get(&url).header("Accept", "*/*")
                                .header("Accept-Language", "zh-CN,zh;q=0.9")
                                .header("Connection", "keep-alive")
                                .header("Sec-Fetch-Mode", "no-cors")
                                .header("Sec-Fetch-Site", "none")
                                .header("Sec-Fetch-User", "?1")
                                .header("Upgrade-Insecure-Requests", "1").send() {
                        Ok(mut response) if response.status().is_success() => {
                            let fullpath = format!("{}/{}", assets_dir, file);
                            if let Some(parent) = Path::new(&fullpath).parent() {
                                let _ = fs::create_dir_all(parent);
                            }
                            let mut file_out = match fs::File::create(&fullpath) {
                                Ok(f) => f,
                                Err(e) => {
                                    pb.finish_with_message(format!("写入失败: {}: {:?}", file, e));
                                    failed_files.lock().unwrap().push(file.clone());
                                    return;
                                }
                            };
                            let mut downloaded: u64 = 0;
                            let mut buf = [0u8; 16 * 1024];
                            let mut last_check = std::time::Instant::now();
                            let mut last_downloaded = 0u64;
                            let mut slow_count = 0;
                            loop {
                                match response.read(&mut buf) {
                                    Ok(0) => break,
                                    Ok(n) => {
                                        if file_out.write_all(&buf[..n]).is_err() {
                                            pb.finish_with_message(format!("写入失败: {}", file));
                                            failed_files.lock().unwrap().push(file.clone());
                                            return;
                                        }
                                        downloaded += n as u64;
                                        pb.set_position(downloaded);
                                        let now = std::time::Instant::now();
                                        if now.duration_since(last_check) >= SPEED_CHECK_INTERVAL {
                                            let bytes = downloaded - last_downloaded;
                                            let speed = bytes as u64 / SPEED_CHECK_INTERVAL.as_secs();
                                            if speed < MIN_SPEED {
                                                slow_count += 1;
                                            } else {
                                                slow_count = 0;
                                            }
                                            last_check = now;
                                            last_downloaded = downloaded;
                                            if slow_count >= 2 {
                                                pb.set_message(format!("速度过慢，切换镜像..."));
                                                break; // 主动中断，进入下一个重试
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        pb.finish_with_message(format!("下载失败: {}: {:?}", file, e));
                                        break;
                                    }
                                }
                            }
                            pb.finish_with_message(format!("已完成: {}", file));
                            success = true;
                            break;
                        }
                        Ok(r) => {
                            println!(
                                "{RED}下载失败: {} 状态码: {}{RESET}",
                                file,
                                r.status()
                            );
                            // 状态异常，重试
                            continue;
                        }
                        Err(e) => {
                            // 请求失败，重试
                            println!("{RED}请求失败: {} 错误: {:?}{RESET}", file, e);
                            continue;
                        }
                    }
                }
                if !success {
                    pb.finish_with_message(format!("请求失败: {}", file));
                    failed_files.lock().unwrap().push(file.clone());
                }
            });
        });
        handles.push(handle);
    }

    for handle in handles {
        let _ = handle.join();
    }
    m.clear().unwrap();

    trash_unindexed_assets(&assets_index, &assets_dir, &trashed_dir)?;

    let failed_files = Arc::try_unwrap(failed_files).unwrap().into_inner().unwrap();
    if !failed_files.is_empty() {
        eprintln!("{RED}以下资源文件下载失败，未完成全部资源更新：{RESET}");
        for file in &failed_files {
            eprintln!("{RED}  - {}{RESET}", file);
        }
        return Err("部分资源文件下载失败".into());
    }

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

/// 从 Gitee 分页获取 commits，直到遇到 local_commit 为止，返回从 local 之后到 remote 的 commit message 列表（旧->新）
fn fetch_commit_logs_gitee(
    owner: &str,
    repo: &str,
    local_commit: &str,
    // remote_commit: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let client = reqwest::blocking::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()?;
    let mut page = 1;
    let per_page = 100usize;
    let mut collected: Vec<(String, String)> = Vec::new(); // (oid, message)

    loop {
        let url = format!(
            "https://gitee.com/api/v5/repos/{owner}/{repo}/commits?page={page}&per_page={per_page}",
            owner = owner,
            repo = repo,
            page = page,
            per_page = per_page
        );

        let resp = client
            .get(&url)
            .header("User-Agent", "Mozilla/5.0")
            .send()?;

        if !resp.status().is_success() {
            break;
        }
        let text = resp.text()?;
        let arr: Value = serde_json::from_str(&text)?;
        let commits = arr
            .as_array()
            .ok_or("Gitee commits response is not array")?;
        if commits.is_empty() {
            break;
        }

        for c in commits {
            // Gitee 的 commit 对象通常有 "id" 与 "message"
            let oid = c
                .get("id")
                .and_then(|v| v.as_str())
                .or_else(|| c.get("sha").and_then(|v| v.as_str()))
                .unwrap_or("")
                .to_string();
            let msg = c
                .get("commit")
                .and_then(|commit| commit.get("message"))
                .or_else(|| c.get("message"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            collected.push((oid.clone(), msg));

            // 一旦遇到 local_commit，停止拉取（因为返回按时间倒序）
            if oid == local_commit {
                break;
            }
        }

        // 如果最后一页包含 local_commit，则停止分页
        if collected.iter().any(|(oid, _)| oid == local_commit) {
            break;
        }
        page += 1;
        // 安全保护：防止无限循环
        if page > 50 {
            break;
        }
    }

    if collected.is_empty() {
        return Ok(Vec::new());
    }

    // collected 目前是从新到旧（Gitee 默认），我们需要从 local 之后到 remote 的顺序（旧->新）
    // 找到 local_commit 在列表中的位置（可能不存在）
    let mut result: Vec<String> = Vec::new();
    // 收集直到但不包含 local_commit；同时只保留到 remote_commit 为止（remote 应位于最前面）
    for (oid, _) in &collected {
        if oid == local_commit {
            break;
        }
    }
    // collected 是 newest -> older，截取从 remote（包含）到 local（不包含）
    // 我们找到索引
    let idx_local = collected.iter().position(|(oid, _)| oid == local_commit);
    // let idx_remote = collected.iter().position(|(oid, _)| oid == remote_commit);

    // let start = idx_remote.unwrap_or(0); // remote 在较前位置（可能 0）
    let start = 0;
    let end = idx_local.unwrap_or(collected.len()); // 不包含 local

    // slice start..end (newest->older), 需要 reverse 成旧->新并格式化
    if start < end {
        let slice = &collected[start..end];
        let mut rev: Vec<(String, String)> = slice.iter().cloned().collect();
        rev.reverse();
        for (oid, msg) in rev {
            let short = if oid.len() >= 8 { &oid[..8] } else { &oid[..] };
            if msg.is_empty() {
                result.push(format!("- ({})", short));
            } else {
                // 只取第一行作为摘要
                let first_line = msg.lines().next().unwrap_or("").trim();
                result.push(format!("- {} ({})", first_line, short));
            }
        }
    }

    Ok(result)
}
