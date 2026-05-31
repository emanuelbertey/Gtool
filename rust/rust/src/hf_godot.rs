use godot::prelude::*;
use hf_hub::{HFClientSync, HFClientBuilder, RepoTypeModel, RepoTypeDataset, RepoTypeSpace};
use hf_hub::repository::AddSource;
use std::path::PathBuf;
use tokio::runtime::Runtime;

#[derive(GodotClass)]
#[class(base=RefCounted)]
pub struct HFGodot {
    client: Option<HFClientSync>,
    base: Base<RefCounted>,
}

#[godot_api]
impl IRefCounted for HFGodot {
    fn init(base: Base<RefCounted>) -> Self {
        Self {
            client: None,
            base,
        }
    }
}

#[godot_api]
impl HFGodot {
    #[func]
    pub fn init_client(&mut self, token: String) -> bool {
        let client_res = HFClientBuilder::new()
            .token(token)
            .build_sync();
        match client_res {
            Ok(client) => {
                self.client = Some(client);
                godot_print!("HFGodot: Client initialized successfully.");
                true
            }
            Err(e) => {
                godot_error!("HFGodot: Failed to initialize client: {:?}", e);
                false
            }
        }
    }

    #[func]
    pub fn upload_file(&self, repo_id: GString, local_file_path: GString, path_in_repo: GString, commit_message: GString, repo_type: GString) -> bool {
        let client = match &self.client {
            Some(c) => c,
            None => {
                godot_error!("HFGodot: Client not initialized.");
                return false;
            }
        };

        let repo_id_str = repo_id.to_string();
        let parts: Vec<&str> = repo_id_str.split('/').collect();
        let (namespace, name) = if parts.len() == 2 {
            (parts[0], parts[1])
        } else {
            ("", repo_id_str.as_str())
        };

        let local_path = local_file_path.to_string();
        let bytes = match std::fs::read(&local_path) {
            Ok(b) => b,
            Err(e) => {
                godot_error!("HFGodot: Failed to read file {}: {:?}", local_path, e);
                return false;
            }
        };

        let path_in_repo_str = path_in_repo.to_string();
        let commit_msg = commit_message.to_string();
        let repo_type_str = repo_type.to_string().to_lowercase();
        
        let res = match repo_type_str.as_str() {
            "dataset" => {
                let repo = client.dataset(namespace, name);
                repo.upload_file()
                    .source(AddSource::bytes(bytes))
                    .path_in_repo(path_in_repo_str)
                    .commit_message(commit_msg)
                    .send()
            }
            "space" => {
                let repo = client.space(namespace, name);
                repo.upload_file()
                    .source(AddSource::bytes(bytes))
                    .path_in_repo(path_in_repo_str)
                    .commit_message(commit_msg)
                    .send()
            }
            _ => {
                let repo = client.model(namespace, name);
                repo.upload_file()
                    .source(AddSource::bytes(bytes))
                    .path_in_repo(path_in_repo_str)
                    .commit_message(commit_msg)
                    .send()
            }
        };

        match res {
            Ok(commit) => {
                godot_print!("HFGodot: Successfully uploaded file. Commit: {:?}", commit.commit_oid);
                true
            }
            Err(e) => {
                godot_error!("HFGodot: Upload failed: {:?}", e);
                false
            }
        }
    }

    #[func]
    pub fn download_file(&self, repo_id: GString, filename: GString, local_dir: GString, repo_type: GString) -> GString {
        let client = match &self.client {
            Some(c) => c,
            None => {
                godot_error!("HFGodot: Client not initialized.");
                return "".into();
            }
        };

        let repo_id_str = repo_id.to_string();
        let parts: Vec<&str> = repo_id_str.split('/').collect();
        let (namespace, name) = if parts.len() == 2 {
            (parts[0], parts[1])
        } else {
            ("", repo_id_str.as_str())
        };

        let filename_str = filename.to_string();
        let local_dir_path = PathBuf::from(local_dir.to_string());
        let repo_type_str = repo_type.to_string().to_lowercase();

        let res = match repo_type_str.as_str() {
            "dataset" => {
                let repo = client.dataset(namespace, name);
                repo.download_file()
                    .filename(filename_str)
                    .local_dir(local_dir_path)
                    .send()
            }
            "space" => {
                let repo = client.space(namespace, name);
                repo.download_file()
                    .filename(filename_str)
                    .local_dir(local_dir_path)
                    .send()
            }
            _ => {
                let repo = client.model(namespace, name);
                repo.download_file()
                    .filename(filename_str)
                    .local_dir(local_dir_path)
                    .send()
            }
        };

        match res {
            Ok(path) => {
                let path_str = path.to_string_lossy().to_string();
                godot_print!("HFGodot: Successfully downloaded file to: {}", path_str);
                GString::from(&path_str)
            }
            Err(e) => {
                godot_error!("HFGodot: Download failed: {:?}", e);
                GString::from("")
            }
        }
    }

    #[func]
    pub fn download_file_range(&self, repo_id: GString, filename: GString, start: i64, end: i64, token: GString, repo_type: GString) -> PackedByteArray {
        let repo_id_str = repo_id.to_string();
        let filename_str = filename.to_string();
        let repo_type_str = repo_type.to_string().to_lowercase();
        let token_str = token.to_string();

        let base_url = match repo_type_str.as_str() {
            "dataset" => "https://huggingface.co/datasets",
            "space" => "https://huggingface.co/spaces",
            _ => "https://huggingface.co",
        };

        let url = format!("{}/{}/resolve/main/{}", base_url, repo_id_str, filename_str);

        let rt = match Runtime::new() {
            Ok(r) => r,
            Err(e) => {
                godot_error!("HFGodot: Failed to create tokio runtime: {:?}", e);
                return PackedByteArray::new();
            }
        };

        let result = rt.block_on(async {
            let client = reqwest::Client::builder()
                .user_agent("Gtool-HFGodot/1.0")
                .build()
                .map_err(|e| format!("{:?}", e))?;

            let mut req = client.get(&url)
                .header("Range", format!("bytes={}-{}", start, end));

            if !token_str.is_empty() {
                req = req.header("Authorization", format!("Bearer {}", token_str));
            }

            let response = req.send().await
                .map_err(|e| format!("Request failed: {:?}", e))?;

            let status = response.status();
            if !status.is_success() {
                return Err(format!("HTTP {}: {}", status, url));
            }

            let data = response.bytes().await
                .map_err(|e| format!("Read failed: {:?}", e))?;

            Ok(data.to_vec())
        });

        match result {
            Ok(bytes) => {
                let mut pba = PackedByteArray::new();
                for &b in &bytes {
                    pba.push_back(b);
                }
                godot_print!("HFGodot: Downloaded {} bytes range [{}-{}] from {}", bytes.len(), start, end, filename_str);
                pba
            }
            Err(e) => {
                godot_error!("HFGodot: Range download failed: {}", e);
                PackedByteArray::new()
            }
        }
    }

    #[func]
    pub fn delete_file(&self, repo_id: GString, path_in_repo: GString, repo_type: GString) -> bool {
        let client = match &self.client {
            Some(c) => c,
            None => {
                godot_error!("HFGodot: Client not initialized.");
                return false;
            }
        };

        let repo_id_str = repo_id.to_string();
        let parts: Vec<&str> = repo_id_str.split('/').collect();
        let (namespace, name) = if parts.len() == 2 {
            (parts[0], parts[1])
        } else {
            ("", repo_id_str.as_str())
        };

        let path_in_repo_str = path_in_repo.to_string();
        let repo_type_str = repo_type.to_string().to_lowercase();

        let res = match repo_type_str.as_str() {
            "dataset" => {
                let repo = client.dataset(namespace, name);
                repo.delete_file().path_in_repo(path_in_repo_str).send()
            }
            "space" => {
                let repo = client.space(namespace, name);
                repo.delete_file().path_in_repo(path_in_repo_str).send()
            }
            _ => {
                let repo = client.model(namespace, name);
                repo.delete_file().path_in_repo(path_in_repo_str).send()
            }
        };

        match res {
            Ok(_) => {
                godot_print!("HFGodot: Successfully deleted file: {}", path_in_repo);
                true
            }
            Err(e) => {
                godot_error!("HFGodot: Delete file failed: {:?}", e);
                false
            }
        }
    }

    #[func]
    pub fn delete_repository(&self, repo_id: GString, repo_type: GString) -> bool {
        let client = match &self.client {
            Some(c) => c,
            None => {
                godot_error!("HFGodot: Client not initialized.");
                return false;
            }
        };

        let repo_id_str = repo_id.to_string();
        let repo_type_str = repo_type.to_string().to_lowercase();

        let res = match repo_type_str.as_str() {
            "dataset" => {
                client.delete_repository()
                    .repo_type(RepoTypeDataset)
                    .repo_id(repo_id_str)
                    .missing_ok(true)
                    .send()
            }
            "space" => {
                client.delete_repository()
                    .repo_type(RepoTypeSpace)
                    .repo_id(repo_id_str)
                    .missing_ok(true)
                    .send()
            }
            _ => {
                client.delete_repository()
                    .repo_type(RepoTypeModel)
                    .repo_id(repo_id_str)
                    .missing_ok(true)
                    .send()
            }
        };

        match res {
            Ok(_) => {
                godot_print!("HFGodot: Successfully deleted repository: {}", repo_id);
                true
            }
            Err(e) => {
                godot_error!("HFGodot: Delete repository failed: {:?}", e);
                false
            }
        }
    }

    #[func]
    pub fn create_repository(&self, repo_id: GString, repo_type: GString, private: bool) -> bool {
        let client = match &self.client {
            Some(c) => c,
            None => {
                godot_error!("HFGodot: Client not initialized.");
                return false;
            }
        };

        let repo_id_str = repo_id.to_string();
        let repo_type_str = repo_type.to_string().to_lowercase();

        let res = match repo_type_str.as_str() {
            "dataset" => {
                client.create_repository()
                    .repo_type(RepoTypeDataset)
                    .repo_id(repo_id_str)
                    .private(private)
                    .exist_ok(true)
                    .send()
            }
            "space" => {
                client.create_repository()
                    .repo_type(RepoTypeSpace)
                    .repo_id(repo_id_str)
                    .private(private)
                    .exist_ok(true)
                    .send()
            }
            _ => {
                client.create_repository()
                    .repo_type(RepoTypeModel)
                    .repo_id(repo_id_str)
                    .private(private)
                    .exist_ok(true)
                    .send()
            }
        };

        match res {
            Ok(_) => {
                godot_print!("HFGodot: Successfully created repository: {}", repo_id);
                true
            }
            Err(e) => {
                godot_error!("HFGodot: Create repository failed: {:?}", e);
                false
            }
        }
    }

    #[func]
    pub fn search_models(&self, author: GString, limit: i32) -> Array<GString> {
        let client = match &self.client {
            Some(c) => c,
            None => {
                godot_error!("HFGodot: Client not initialized.");
                return Array::new();
            }
        };

        let author_str = author.to_string();
        let limit_size = if limit > 0 { limit as usize } else { 10 };

        let res = client
            .list_models()
            .author(author_str)
            .limit(limit_size)
            .send();

        let mut results = Array::new();
        match res {
            Ok(models) => {
                for model in models {
                    results.push(&GString::from(&model.id));
                }
            }
            Err(e) => {
                godot_error!("HFGodot: List models failed: {:?}", e);
            }
        }
        results
    }

    #[func]
    pub fn repo_exists(&self, repo_id: GString, repo_type: GString) -> bool {
        let client = match &self.client {
            Some(c) => c,
            None => {
                godot_error!("HFGodot: Client not initialized.");
                return false;
            }
        };

        let repo_id_str = repo_id.to_string();
        let parts: Vec<&str> = repo_id_str.split('/').collect();
        let (namespace, name) = if parts.len() == 2 {
            (parts[0], parts[1])
        } else {
            ("", repo_id_str.as_str())
        };

        let repo_type_str = repo_type.to_string().to_lowercase();

        let res = match repo_type_str.as_str() {
            "dataset" => {
                client.dataset(namespace, name).exists().send()
            }
            "space" => {
                client.space(namespace, name).exists().send()
            }
            _ => {
                client.model(namespace, name).exists().send()
            }
        };

        match res {
            Ok(exists) => exists,
            Err(e) => {
                godot_error!("HFGodot: Exists check failed: {:?}", e);
                false
            }
        }
    }

    #[func]
    pub fn file_exists(&self, repo_id: GString, filename: GString, repo_type: GString) -> bool {
        let client = match &self.client {
            Some(c) => c,
            None => {
                godot_error!("HFGodot: Client not initialized.");
                return false;
            }
        };

        let repo_id_str = repo_id.to_string();
        let parts: Vec<&str> = repo_id_str.split('/').collect();
        let (namespace, name) = if parts.len() == 2 {
            (parts[0], parts[1])
        } else {
            ("", repo_id_str.as_str())
        };

        let filename_str = filename.to_string();
        let repo_type_str = repo_type.to_string().to_lowercase();

        let res = match repo_type_str.as_str() {
            "dataset" => {
                client.dataset(namespace, name)
                    .file_exists()
                    .filename(filename_str)
                    .send()
            }
            "space" => {
                client.space(namespace, name)
                    .file_exists()
                    .filename(filename_str)
                    .send()
            }
            _ => {
                client.model(namespace, name)
                    .file_exists()
                    .filename(filename_str)
                    .send()
            }
        };

        match res {
            Ok(exists) => exists,
            Err(e) => {
                godot_error!("HFGodot: File exists check failed: {:?}", e);
                false
            }
        }
    }

    #[func]
    pub fn list_repo_files(&self, repo_id: GString, recursive: bool, repo_type: GString) -> Array<GString> {
        let client = match &self.client {
            Some(c) => c,
            None => {
                godot_error!("HFGodot: Client not initialized.");
                return Array::new();
            }
        };

        let repo_id_str = repo_id.to_string();
        let parts: Vec<&str> = repo_id_str.split('/').collect();
        let (namespace, name) = if parts.len() == 2 {
            (parts[0], parts[1])
        } else {
            ("", repo_id_str.as_str())
        };

        let repo_type_str = repo_type.to_string().to_lowercase();

        let res = match repo_type_str.as_str() {
            "dataset" => {
                client.dataset(namespace, name)
                    .list_tree()
                    .recursive(recursive)
                    .send()
            }
            "space" => {
                client.space(namespace, name)
                    .list_tree()
                    .recursive(recursive)
                    .send()
            }
            _ => {
                client.model(namespace, name)
                    .list_tree()
                    .recursive(recursive)
                    .send()
            }
        };

        let mut results = Array::new();
        match res {
            Ok(entries) => {
                for entry in entries {
                    match entry {
                        hf_hub::repository::RepoTreeEntry::File { path, .. } => {
                            results.push(&GString::from(&path));
                        }
                        hf_hub::repository::RepoTreeEntry::Directory { path, .. } => {
                            results.push(&GString::from(&path));
                        }
                    }
                }
            }
            Err(e) => {
                godot_error!("HFGodot: List tree failed: {:?}", e);
            }
        }
        results
    }
}
