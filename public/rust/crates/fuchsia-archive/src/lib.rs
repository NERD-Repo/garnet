#![allow(dead_code)]
extern crate bincode;
extern crate failure;
extern crate tempdir;
#[macro_use]
extern crate serde_derive;
extern crate serde;

use bincode::{serialize_into, Infinite};
use failure::Error;
use std::collections::BTreeMap;
use std::io::{Seek, Write};

const MAGIC_INDEX_VALUE: [u8; 8] = [0xc8, 0xbf, 0x0b, 0x48, 0xad, 0xab, 0xc5, 0x11];

type ChunkType = u64;

const HASH_CHUNK: ChunkType = 0;
const DIR_HASH_CHUNK: ChunkType = 0x2d48534148524944; // "DIRHASH-"
const DIR_CHUNK: ChunkType = 0x2d2d2d2d2d524944; // "DIR-----"
const DIR_NAMES_CHUNK: ChunkType = 0x53454d414e524944; // "DIRNAMES"

#[derive(Serialize)]
struct Index {
    magic: [u8; 8],
    length: u64,
}

struct IndexEntry {
    chunk_type: ChunkType,
    offset: u64,
    length: u64,
}

struct DirectoryEntry {
    name_offset: u32,
    name_length: u16,
    reserved: u16,
    data_offset: u64,
    data_length: u64,
    reserved2: u64,
}

pub fn write<T>(target: &mut T, inputs: &mut Iterator<Item = (&str, &str)>) -> Result<(), Error>
where
    T: Write + Seek,
{
    let mut input_map: BTreeMap<&str, &str> = BTreeMap::new();
    for (key, value) in inputs {
        input_map.insert(key, value);
    }

    let index = Index {
        magic: MAGIC_INDEX_VALUE,
        length: 0,
    };
    serialize_into(target, &index, Infinite)?;
    Ok(())
}

#[cfg(test)]
mod tests {

    use failure::Error;
    use std::collections::HashMap;
    use std::fs::File;
    use std::fs::create_dir_all;
    use std::io::{Cursor, Write};
    use tempdir::TempDir;
    use {write, MAGIC_INDEX_VALUE};

    fn create_test_files(file_names: &[&str]) -> Result<TempDir, Error> {
        let tmp_dir = TempDir::new("fuchsia_archive_test")?;
        for file_name in file_names {
            let file_path = tmp_dir.path().join(file_name);
            let parent_dir = file_path.parent().unwrap();
            create_dir_all(&parent_dir)?;
            let file_path = tmp_dir.path().join(file_name);
            let mut tmp_file = File::create(&file_path)?;
            writeln!(tmp_file, "{}", file_path.to_string_lossy())?;
        }
        Ok(tmp_dir)
    }

    fn example_archive() -> Vec<u8> {
        let mut b: Vec<u8> = vec![0; 16384];
        let header = vec![
            0xc8, 0xbf, 0x0b, 0x48, 0xad, 0xab, 0xc5, 0x11, 0x30, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x44, 0x49, 0x52, 0x2d, 0x2d, 0x2d, 0x2d, 0x2d, 0x40, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x60, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x44, 0x49,
            0x52, 0x4e, 0x41, 0x4d, 0x45, 0x53, 0xa0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00,
            0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00,
            0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x20, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00, 0x00, 0x30, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x61, 0x62, 0x64, 0x69, 0x72, 0x2f, 0x63, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        b[0..header.len()].copy_from_slice(header.as_slice());
        let name_a = b"a\n";
        let a_loc = 4096;
        b[a_loc..a_loc + name_a.len()].copy_from_slice(name_a);
        let name_b = b"b\n";
        let b_loc = 4096;
        b[b_loc..b_loc + name_b.len()].copy_from_slice(name_b);
        let name_c = b"dir/c\n";
        let c_loc = 12288;
        b[c_loc..c_loc + name_c.len()].copy_from_slice(name_c);
        b
    }

    #[test]
    fn test_write() {
        let files = ["a", "b", "dir/c"];
        let test_dir = create_test_files(&files).unwrap();
        let mut inputs: HashMap<String, String> = HashMap::new();
        for file_name in files.iter() {
            let path = test_dir
                .path()
                .join(file_name)
                .to_string_lossy()
                .to_string();
            inputs.insert(file_name.to_string(), path);
        }
        let mut target = Cursor::new(Vec::new());
        write(
            &mut target,
            &mut inputs.iter().map(|(a, b)| (a.as_str(), b.as_str())),
        ).unwrap();
        assert!(target.get_ref()[0..8] == MAGIC_INDEX_VALUE);
        assert_eq!(*target.get_ref(), example_archive());
    }
}
