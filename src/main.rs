use std::error::Error;
use std::{fs, io};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use std::process::Command as SystemCommand;

use clap::Command as ClapCommand;
use reqwest::Client;
use serde::{Deserialize, Serialize};

// toml file config
#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Config {
    bin_path: String,
    commands: Vec<Command>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Command {
    name: String,
    org: String,
    repository: String,
    bin_part_name: String,
}

// for response of GitHub API
#[derive(Debug, Serialize, Deserialize)]
struct ReleaseResponse {
    assets: Vec<Asset>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    content_type: String,
}

// sub commands
const SUBCOMMANDS_UPDATE_ALL: &str = "update_all";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let matches = ClapCommand::new("my CLI management tools")
        .version("1.0")
        .about("my CLI management tools")
        .subcommand(ClapCommand::new(SUBCOMMANDS_UPDATE_ALL)
            .about("install or update all commands"))
        .get_matches();

    if let Some(_list) = matches.subcommand_matches(SUBCOMMANDS_UPDATE_ALL) {
        let mut unzip_command = SystemCommand::new("unzip");
        let mut tar_command = SystemCommand::new("tar");

        // TODO: need to check existing commands
        // unzip_command.arg("--help").output().expect("Not found unzip command");
        // tar_command.arg("--help").output().expect("Not found tar command");

        let file = File::open("sample.yml").expect("Not found config file");
        let mut buf_reader = BufReader::new(file);
        let mut contents = String::new();
        buf_reader.read_to_string(&mut contents).expect("Unread buffer");

        let config: Config = serde_yaml::from_str(&contents).expect("Failed to parse from yaml");

        for (_pos, command) in config.commands.iter().enumerate() {
            let org = &command.org;
            let repo = &command.repository;

            let url = format!("https://api.github.com/repos/{}/{}/releases/latest", org, repo);

            let client = Client::new();
            let res = client.get(url).header("User-Agent", "Awesome-Octocat-App")
                .send()
                .await
                .expect("Failed to request GitHub API");

            let json = res.json::<ReleaseResponse>().await.expect("Failed to read json");
            let download_assets = json.assets.into_iter()
                .filter(|asset|asset.browser_download_url.contains(&command.bin_part_name))
                .collect::<Vec<Asset>>();

            match download_assets.first() {
                Some(asset) => {
                    let response = client.get(&asset.browser_download_url)
                        .send()
                        .await
                        .expect("Failed to download archive file");
                    let bytes = response.bytes().await?;
                    let mut file = File::create(&asset.name)?;
                    io::copy(&mut bytes.as_ref(), &mut file)?;

                    match &*asset.content_type.to_string() {
                        "application/zip" => {
                            unzip_command.args(["-d", &command.name, "-o", &asset.name])
                                .output()
                                .expect("Failed to unzip");
                            let path = Path::new(&config.bin_path);
                            fs::rename(format!("{}/{}", &command.name, &command.name), path.join(&command.name))
                                .expect("Failed to move file")
                        }
                        "application/gzip" => {
                            fs::create_dir(&command.name)?;
                            tar_command.args(["-mxvf", &asset.name, "-C", &command.name, "--strip-components", "1"])
                                .output()
                                .expect("Failed to tar");
                            let path = Path::new(&config.bin_path);
                            fs::rename(format!("{}/{}", &command.name, &command.name), path.join(&command.name))
                                .expect("Failed to move file")
                        }
                        _ => println!("wtf"),
                    }

                    fs::remove_dir_all(&command.name)
                        .expect(&format!("Failed to remove directory: {}", &command.name));
                    fs::remove_file(&asset.name)
                        .expect(&format!("Failed to remove archive file: {}", &asset.name));
                }
                None => println!("Maybe it does not set latest on GitHub Releases")
            }
        }
    }
    Ok(())
}
