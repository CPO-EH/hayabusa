use std::collections::HashMap;
use std::io::BufRead;
use std::path::Path;
use std::{fs, io};
use yaml_rust2::yaml::Hash;
use yaml_rust2::Yaml;

fn process_value(value: &Yaml, replacements: &HashMap<String, Vec<String>>) -> Yaml {
    match value {
        Yaml::String(val_str) => {
            let mut replaced_values = vec![];
            for (placeholder, replace_list) in replacements {
                if val_str.contains(placeholder) {
                    replaced_values.extend(
                        replace_list
                            .iter()
                            .cloned()
                            .map(|s| Yaml::String(val_str.replace(placeholder, &s))),
                    );
                }
            }
            if replaced_values.is_empty() {
                Yaml::String(val_str.clone())
            } else {
                Yaml::Array(replaced_values)
            }
        }
        Yaml::Array(array) => {
            let new_array: Vec<Yaml> = array
                .iter()
                .map(|item| process_yaml(item, replacements).0)
                .collect();
            Yaml::Array(new_array)
        }
        _ => process_yaml(value, replacements).0,
    }
}

pub fn process_yaml(
    yaml: &Yaml,
    replacements: &HashMap<String, Vec<String>>,
) -> (Yaml, bool, bool) {
    let mut expand_found = false;
    let mut expand_enabled = false;
    match yaml {
        Yaml::Hash(hash) => {
            let mut new_hash = Hash::new();
            for (key, value) in hash {
                if let Yaml::String(key_str) = key {
                    if key_str.contains("|expand") {
                        expand_found = true;
                        let new_key = key_str.replace("|expand", "");
                        let org_value = value.clone();
                        let new_value = process_value(&org_value, replacements);
                        if org_value != new_value {
                            expand_enabled = true;
                        };
                        new_hash.insert(Yaml::String(new_key), new_value);
                    } else {
                        new_hash.insert(key.clone(), process_yaml(value, replacements).0);
                    }
                } else {
                    new_hash.insert(key.clone(), process_yaml(value, replacements).0);
                }
            }
            (Yaml::Hash(new_hash), expand_found, expand_enabled)
        }
        Yaml::Array(array) => {
            let new_array: Vec<Yaml> = array
                .iter()
                .map(|item| process_yaml(item, replacements).0)
                .collect();
            (Yaml::Array(new_array), expand_found, expand_enabled)
        }
        _ => (yaml.clone(), expand_found, expand_enabled),
    }
}

pub fn read_expand_files<P: AsRef<Path>>(dir: P) -> io::Result<HashMap<String, Vec<String>>> {
    let mut expand_map = HashMap::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().map_or(false, |ext| ext == "txt") {
                if let Some(key) = path.file_stem().and_then(|s| s.to_str()) {
                    let file = fs::File::open(&path)?;
                    let reader = io::BufReader::new(file);
                    let values: Vec<String> = reader
                        .lines()
                        .map_while(Result::ok)
                        .map(|s| s.trim().to_string())
                        .collect();
                    if !values.is_empty() {
                        expand_map.insert(format!("%{}%", key), values);
                    }
                }
            }
        }
        if expand_map.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "No expand.txt files found",
            ));
        }
    }
    Ok(expand_map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use yaml_rust2::YamlLoader;

    #[test]
    fn test_process_yaml() {
        let yaml_str = r#"
        key1: value1
        key2|expand: "test%placeholder%"
        key3:
          subkey1|expand: "%placeholder%"
          subkey2: subvalue2
        key4|expand:
          - item1
        "#;

        let docs = YamlLoader::load_from_str(yaml_str).unwrap();
        let mut replacements = HashMap::new();
        replacements.insert(
            "%placeholder%".to_string(),
            vec!["replace_value1".to_string(), "replace_value2".to_string()],
        );
        let processed_yaml = process_yaml(&docs[0], &replacements);

        let expected_yaml_str = r#"
        key1: value1
        key2: [testreplace_value1, testreplace_value2]
        key3:
          subkey1: [replace_value1, replace_value2]
          subkey2: subvalue2
        key4:
          - item1
        "#;

        let expected_docs = YamlLoader::load_from_str(expected_yaml_str).unwrap();
        assert!(processed_yaml.1);
        assert_eq!(processed_yaml.0, expected_docs[0]);
    }

    #[test]
    fn test_process_value() {
        let yaml_str = r#"
        key2|expand: "test%placeholder%"
        "#;

        let docs = YamlLoader::load_from_str(yaml_str).unwrap();
        let mut replacements = HashMap::new();
        replacements.insert(
            "%placeholder%".to_string(),
            vec!["replace_value1".to_string(), "replace_value2".to_string()],
        );

        // Test process_value directly
        let value = &docs[0]["key2|expand"];
        let processed_value = process_value(value, &replacements);

        let expected_value = Yaml::Array(vec![
            Yaml::String("testreplace_value1".to_string()),
            Yaml::String("testreplace_value2".to_string()),
        ]);

        assert_eq!(processed_value, expected_value);
    }
}
