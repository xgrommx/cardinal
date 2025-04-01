use cardinal_sdk::{
    fs_visit::{Node, WalkData, walk_it},
    name_pool::NamePool,
};
use mimalloc::MiMalloc;
use slab::Slab;
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::BufWriter,
    path::PathBuf,
    sync::Arc,
    time::Instant,
};

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

struct SlabNode {
    parent: Option<usize>,
    children: Vec<usize>,
    name: String,
}

fn construct_nodex_graph(
    parent: Option<usize>,
    node: &Arc<Node>,
    slab: &mut Slab<SlabNode>,
) -> usize {
    let slab_node = SlabNode {
        parent,
        children: vec![],
        name: node.data.name.clone(),
    };
    let index = slab.insert(slab_node);
    slab[index].children = node
        .children
        .iter()
        .map(|node| construct_nodex_graph(Some(index), node, slab))
        .collect();
    index
}

/// Combine the construction routine of NamePool and BTreeMap since we can deduplicate node name for free.
// TODO(ldm0): Memory optimization can be done by letting name index reference the name in the pool(gc need to be considered though)
fn construct_name_index_and_namepool(
    node: &Arc<Node>,
    name_index: &mut BTreeMap<String, Vec<Arc<Node>>>,
    name_pool: &mut NamePool,
) {
    if let Some(nodes) = name_index.get_mut(&node.data.name) {
        nodes.push(node.clone());
    } else {
        name_pool.push(&node.data.name);
        name_index.insert(node.data.name.clone(), vec![node.clone()]);
    };
    for node in &node.children {
        construct_name_index_and_namepool(&node, name_index, name_pool);
    }
}

fn main() {
    // first multithreaded walk the file system then get a simple tree structure
    let walk_data = WalkData::default();
    let visit_time = Instant::now();
    let node = walk_it(PathBuf::from("/"), &walk_data).expect("failed to walk");
    let node = Arc::new(node);
    dbg!(walk_data);
    dbg!(visit_time.elapsed());

    // next construct the node graph which is single threaded but allows cross referencing
    {
        let slab_time = Instant::now();
        let mut slab = Slab::new();
        let node_graph = construct_nodex_graph(None, &node, &mut slab);
        dbg!(slab_time.elapsed());
        dbg!(node_graph);
        dbg!(slab.len());
    }

    {
        let name_index_time = Instant::now();
        let mut name_index = BTreeMap::default();
        let mut name_pool = NamePool::new();
        construct_name_index_and_namepool(&node, &mut name_index, &mut name_pool);
        dbg!(name_index_time.elapsed());
        dbg!(name_index.len());

        let search_time = Instant::now();
        for (i, name) in name_pool.search_substr("athbyt").enumerate() {
            // TODO(ldm0): this can be parallelized
            if let Some(nodes) = name_index.get(name) {
                for node in nodes {
                    println!("[{}] key: {}", i, node.data.name);
                }
            }
        }
        dbg!(name_pool.len() / 1024 / 1024);
        dbg!(search_time.elapsed());
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
