#![allow(dead_code, unused, unused_mut)]
extern crate bincode;
#[macro_use]
extern crate failure;
extern crate itertools;
extern crate tempdir;
#[macro_use]
extern crate serde_derive;
extern crate serde;

use bincode::{deserialize_from, serialize_into, Infinite};
use failure::Error;
use std::collections::BTreeMap;
use std::fs;
use std::fs::File;
use std::io::{copy, Read, Write};

const MAGIC_INDEX_VALUE: [u8; 8] = [0xc8, 0xbf, 0x0b, 0x48, 0xad, 0xab, 0xc5, 0x11];

type ChunkType = u64;

const HASH_CHUNK: ChunkType = 0;
const DIR_HASH_CHUNK: ChunkType = 0x2d48534148524944; // "DIRHASH-"
const DIR_CHUNK: ChunkType = 0x2d2d2d2d2d524944; // "DIR-----"
const DIR_NAMES_CHUNK: ChunkType = 0x53454d414e524944; // "DIRNAMES"

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Index {
    magic: [u8; 8],
    length: u64,
}

const INDEX_LEN: u64 = 8 + 8;

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct IndexEntry {
    chunk_type: ChunkType,
    offset: u64,
    length: u64,
}

const INDEX_ENTRY_LEN: u64 = 8 + 8 + 8;

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct DirectoryEntry {
    name_offset: u32,
    name_length: u16,
    reserved: u16,
    data_offset: u64,
    data_length: u64,
    reserved2: u64,
}

const DIRECTORY_ENTRY_LEN: u64 = 4 + 2 + 2 + 8 + 8 + 8;
const CONTENT_ALIGNMENT: u64 = 4096;

#[derive(Debug, Fail)]
#[fail(display = "Invalid archive")]
pub struct InvalidArchive {}

fn write_zeros<T>(target: &mut T, count: usize) -> Result<(), Error>
where
    T: Write,
{
    println!("write_zeros count = {}", count);
    let b: Vec<u8> = vec![0; count];
    target.write_all(&b)?;
    Ok(())
}

pub fn write<T>(target: &mut T, inputs: &mut Iterator<Item = (&str, &str)>) -> Result<(), Error>
where
    T: Write,
{
    let mut input_map: BTreeMap<&str, &str> = BTreeMap::new();
    for (destination_name, source_name) in inputs {
        input_map.insert(destination_name, source_name);
    }

    let mut path_data: Vec<u8> = vec![];
    let mut entries = vec![];
    for (destination_name, source_name) in input_map.iter() {
        let metadata = fs::metadata(source_name)?;
        entries.push(DirectoryEntry {
            name_offset: path_data.len() as u32,
            name_length: destination_name.len() as u16,
            reserved: 0,
            data_offset: 0,
            data_length: metadata.len(),
            reserved2: 0,
        });
        path_data.extend_from_slice(destination_name.as_bytes());
    }

    let index = Index {
        magic: MAGIC_INDEX_VALUE,
        length: 2 * INDEX_ENTRY_LEN as u64,
    };

    let dir_index = IndexEntry {
        chunk_type: DIR_CHUNK as u64,
        offset: INDEX_LEN + INDEX_ENTRY_LEN * 2,
        length: entries.len() as u64 * DIRECTORY_ENTRY_LEN,
    };

    let name_index = IndexEntry {
        chunk_type: DIR_NAMES_CHUNK as u64,
        offset: dir_index.offset + dir_index.length,
        length: align(path_data.len() as u64, 8),
    };

    serialize_into(target, &index, Infinite)?;

    serialize_into(target, &dir_index, Infinite)?;

    serialize_into(target, &name_index, Infinite)?;

    let mut content_offset = align(name_index.offset + name_index.length, CONTENT_ALIGNMENT);

    for ref mut entry in &mut entries {
        entry.data_offset = content_offset;
        content_offset = align(content_offset + entry.data_length, CONTENT_ALIGNMENT);
        serialize_into(target, &entry, Infinite)?;
    }

    target.write(&path_data)?;

    write_zeros(target, name_index.length as usize - path_data.len())?;

    let pos = name_index.offset + name_index.length;
    let padding_count = align(pos, CONTENT_ALIGNMENT) - pos;
    write_zeros(target, padding_count as usize)?;

    let mut entry_index = 0;
    for (_, source_name) in input_map.iter() {
        let mut f = File::open(source_name)?;
        copy(&mut f, target)?;
        let pos = entries[entry_index].data_offset + entries[entry_index].data_length;
        let padding_count = align(pos, CONTENT_ALIGNMENT) - pos;
        entry_index = entry_index + 1;
        write_zeros(target, padding_count as usize)?;
    }

    Ok(())
}

struct Reader {}

impl Reader {
    pub fn new<T>(source: &mut T) -> Result<Reader, Error>
    where
        T: Read,
    {
        let mut reader = Reader {};
        let index = Reader::read_index(source)?;
        let (dir_index, dir_name_index) =
            Reader::read_index_entries(source, index.length / INDEX_ENTRY_LEN, &index)?;
        if dir_index.is_none() {
            return Err(format_err!("Invalid archive, missing directory index"));
        }
        let dir_index = dir_index.unwrap();
        if dir_name_index.is_none() {
            return Err(format_err!("Invalid archive, missing directory name index"));
        }
        let dir_name_index = dir_name_index.unwrap();
        Ok(reader)
    }

    fn read_index<T>(source: &mut T) -> Result<Index, Error>
    where
        T: Read,
    {
        let decoded_index: Index = deserialize_from(source, Infinite)?;
        if decoded_index.magic != MAGIC_INDEX_VALUE {
            Err(format_err!("Invalid archive, bad magic"))
        } else {
            if decoded_index.length % INDEX_ENTRY_LEN != 0 {
                Err(format_err!("Invalid archive, bad index length"))
            } else {
                Ok(decoded_index)
            }
        }
    }

    fn read_index_entries<T>(
        source: &mut T,
        count: u64,
        index: &Index,
    ) -> Result<(Option<Index>, Option<Index>), Error>
    where
        T: Read,
    {
        let mut dir_index: Option<Index> = None;
        let mut dir_name_index: Option<Index> = None;
        let mut entries: Vec<IndexEntry> = vec![];
        for _index in [0..count].iter() {
            let entry: IndexEntry = deserialize_from(source, Infinite)?;
            match entry.chunk_type {
                _ => {}
            }
            match entries.last() {
                None => {}
                Some(ref last_entry) => {
                    if last_entry.chunk_type > entry.chunk_type {
                        return Err(format_err!("Invalid archive, invalid index entry order"));
                    }
                }
            }
            if entry.offset < index.length {
                return Err(format_err!("Invalid archive, short offset"));
            }
            entries.push(entry);
        }
        Ok((dir_index, dir_name_index))
    }
}

// align rounds i up to a multiple of n
fn align(unrounded_value: u64, multiple: u64) -> u64 {
    let rem = unrounded_value.checked_rem(multiple).unwrap();
    if rem > 0 {
        unrounded_value - rem + multiple
    } else {
        unrounded_value
    }
}

#[cfg(test)]
mod tests {

    use bincode::{deserialize_from, serialize_into, Infinite};
    use failure::Error;
    use itertools::assert_equal;
    use std::collections::HashMap;
    use std::fs::File;
    use std::fs::create_dir_all;
    use std::io::{Cursor, Seek, SeekFrom, Write};
    use tempdir::TempDir;
    use {align, write, DirectoryEntry, Index, IndexEntry, Reader, DIRECTORY_ENTRY_LEN, DIR_CHUNK,
         INDEX_ENTRY_LEN, INDEX_LEN, MAGIC_INDEX_VALUE};

    fn create_test_files(file_names: &[&str]) -> Result<TempDir, Error> {
        let tmp_dir = TempDir::new("fuchsia_archive_test")?;
        for file_name in file_names {
            let file_path = tmp_dir.path().join(file_name);
            let parent_dir = file_path.parent().unwrap();
            create_dir_all(&parent_dir)?;
            let file_path = tmp_dir.path().join(file_name);
            let mut tmp_file = File::create(&file_path)?;
            writeln!(tmp_file, "{}", file_name)?;
        }
        Ok(tmp_dir)
    }

    fn example_archive() -> Vec<u8> {
        let mut b: Vec<u8> = vec![0; 16384];
        let header = vec![
            // magic
            0xc8, 0xbf, 0x0b, 0x48, 0xad, 0xab, 0xc5, 0x11,
            // length of index entries
            0x30, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            // chunk type
            0x44, 0x49, 0x52, 0x2d, 0x2d, 0x2d, 0x2d, 0x2d,
            // offset to chunk
            0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            // length of chunk
            0x60, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            // chunk type
            0x44, 0x49, 0x52, 0x4e, 0x41, 0x4d, 0x45, 0x53,
            // offset to chunk
            0xa0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            // length of chunk
            0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
            0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
            0x00, 0x20, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x02, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00,
            0x00, 0x30, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x61, 0x62, 0x64, 0x69, 0x72, 0x2f, 0x63, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        b[0..header.len()].copy_from_slice(header.as_slice());
        let name_a = b"a\n";
        let a_loc = 4096;
        b[a_loc..a_loc + name_a.len()].copy_from_slice(name_a);
        let name_b = b"b\n";
        let b_loc = 8192;
        b[b_loc..b_loc + name_b.len()].copy_from_slice(name_b);
        let name_c = b"dir/c\n";
        let c_loc = 12288;
        b[c_loc..c_loc + name_c.len()].copy_from_slice(name_c);
        b
    }

    #[test]
    fn test_write() {
        let files = ["b", "a", "dir/c"];
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
        let example_archive = example_archive();
        let target_ref = target.get_ref();
        assert_equal(target_ref, &example_archive);
        assert_eq!(*target_ref, example_archive);
    }

    #[test]
    fn test_serialize_index() {
        let mut target = Cursor::new(Vec::new());
        let index = Index {
            magic: MAGIC_INDEX_VALUE,
            length: 2 * INDEX_ENTRY_LEN as u64,
        };
        serialize_into(&mut target, &index, Infinite).unwrap();
        assert_eq!(target.get_ref().len() as u64, INDEX_LEN);
        target.seek(SeekFrom::Start(0)).unwrap();

        let decoded_index: Index = deserialize_from(&mut target, Infinite).unwrap();
        assert_eq!(index, decoded_index);
    }

    #[test]
    fn test_serialize_index_entry() {
        let mut target = Cursor::new(Vec::new());
        let index_entry = IndexEntry {
            chunk_type: DIR_CHUNK as u64,
            offset: 999,
            length: 444,
        };
        serialize_into(&mut target, &index_entry, Infinite).unwrap();
        assert_eq!(target.get_ref().len() as u64, INDEX_ENTRY_LEN);
        target.seek(SeekFrom::Start(0)).unwrap();

        let decoded_index_entry: IndexEntry = deserialize_from(&mut target, Infinite).unwrap();
        assert_eq!(index_entry, decoded_index_entry);
    }

    #[test]
    fn test_serialize_directory_entry() {
        let mut target = Cursor::new(Vec::new());
        let index_entry = DirectoryEntry {
            name_offset: 33,
            name_length: 66,
            reserved: 0,
            data_offset: 99,
            data_length: 1011,
            reserved2: 0,
        };
        serialize_into(&mut target, &index_entry, Infinite).unwrap();
        assert_eq!(target.get_ref().len() as u64, DIRECTORY_ENTRY_LEN);
        target.seek(SeekFrom::Start(0)).unwrap();

        let decoded_index_entry: DirectoryEntry = deserialize_from(&mut target, Infinite).unwrap();
        assert_eq!(index_entry, decoded_index_entry);
    }

    #[test]
    fn test_align_values() {
        assert_eq!(align(3, 8), 8);
        assert_eq!(align(13, 8), 16);
        assert_eq!(align(16, 8), 16);
    }

    #[test]
    #[should_panic]
    fn test_align_zero() {
        align(3, 0);
    }

    fn corrupt_magic(b: &mut Vec<u8>) {
        b[0] = 0;
    }

    fn corrupt_index_length(b: &mut Vec<u8>) {
        let v: u64 = 1;
        let mut cursor = Cursor::new(b);
        cursor.seek(SeekFrom::Start(8)).unwrap();
        serialize_into(&mut cursor, &v, Infinite).unwrap();
    }

    fn corrupt_dir_index_type(b: &mut Vec<u8>) {
        let v: u8 = 255;
        let mut cursor = Cursor::new(b);
        cursor.seek(SeekFrom::Start(INDEX_LEN)).unwrap();
        serialize_into(&mut cursor, &v, Infinite).unwrap();
    }

    #[test]
    fn test_reader() {
        let example = example_archive();
        let mut example_cursor = Cursor::new(&example);
        let mut reader = Reader::new(&mut example_cursor).unwrap();

        let corrupters = [corrupt_magic, corrupt_index_length, corrupt_dir_index_type];
        let mut index = 0;
        for corrupter in corrupters.iter() {
            let mut example = example_archive();
            corrupter(&mut example);
            let mut example_cursor = Cursor::new(&mut example);
            let mut reader = Reader::new(&mut example_cursor);
            assert!(reader.is_err(), "corrupter index = {}", index);
            index += 1;
        }
    }
}
