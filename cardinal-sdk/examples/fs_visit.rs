use cardinal_sdk::fs_visit::{Node, WalkData, walk_it};
use std::{
    collections::HashSet,
    fs::{self, File},
    io::BufWriter,
    path::PathBuf,
    time::Instant,
};

fn push_names(node: &Node, names: &mut HashSet<String>) {
    names.insert(node.name.clone());
    for child in &node.children {
        names.insert(node.name.clone());
        for child in &child.children {
            push_names(&child, names);
        }
    }
}

fn main() {
    let walk_data = WalkData::default();
    let visit_time = Instant::now();
    let node = walk_it(PathBuf::from("/"), &walk_data).expect("failed to walk");
    dbg!(walk_data);
    dbg!(visit_time.elapsed());

    {
        let names_time = Instant::now();
        let mut names = HashSet::new();
        push_names(&node, &mut names);
        dbg!(names_time.elapsed());
        dbg!(names.len());
    }

    {
        let cbor_time = Instant::now();
        let output = File::create("target/tree.cbor").unwrap();
        let mut output = BufWriter::new(output);
        cbor4ii::serde::to_writer(&mut output, &node).unwrap();
        dbg!(cbor_time.elapsed());
        dbg!(fs::metadata("target/tree.cbor").unwrap().len() / 1024 / 1024);
    }

    {
        let bincode_time = Instant::now();
        let output = File::create("target/tree.bin").unwrap();
        let mut output = BufWriter::new(output);
        bincode::encode_into_std_write(&node, &mut output, bincode::config::standard()).unwrap();
        dbg!(bincode_time.elapsed());
        dbg!(fs::metadata("target/tree.bin").unwrap().len() / 1024 / 1024);
    }

    {
        let zstd_bincode_time = Instant::now();
        let output = File::create("target/tree.bin.zstd").unwrap();
        let mut output = zstd::Encoder::new(output, 3).unwrap();
        bincode::encode_into_std_write(&node, &mut output, bincode::config::standard()).unwrap();
        dbg!(zstd_bincode_time.elapsed());
        dbg!(fs::metadata("target/tree.bin.zstd").unwrap().len() / 1024 / 1024);
    }
}
