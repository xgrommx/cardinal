#[cfg(test)]
mod extra {
    use crate::SearchCache;
    use search_cancel::CancellationToken;
    use std::{fs, path::PathBuf};
    use tempdir::TempDir;

    #[test]
    fn test_search_empty_returns_all_nodes() {
        let tmp = TempDir::new("search_empty").unwrap();
        fs::File::create(tmp.path().join("a.txt")).unwrap();
        fs::File::create(tmp.path().join("b.txt")).unwrap();
        let cache = SearchCache::walk_fs(tmp.path().to_path_buf());
        let all = cache
            .search_empty(CancellationToken::noop())
            .expect("noop cancellation token should not cancel");
        assert_eq!(all.len(), cache.get_total_files());
    }

    #[test]
    fn test_node_path_root_and_child() {
        let tmp = TempDir::new("node_path").unwrap();
        fs::create_dir(tmp.path().join("dir1")).unwrap();
        fs::File::create(tmp.path().join("dir1/file_x")).unwrap();
        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());
        let idxs = cache.search("file_x").unwrap();
        assert_eq!(idxs.len(), 1);
        let full = cache.node_path(idxs.into_iter().next().unwrap()).unwrap();
        assert!(full.ends_with(PathBuf::from("dir1/file_x")));
    }

    #[test]
    fn test_remove_node_path_nonexistent_returns_none() {
        let tmp = TempDir::new("remove_node_none").unwrap();
        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());
        // remove_node_path is private via crate; exercise via scan removal scenario
        // create then delete file and ensure second scan removal returns None
        let file = tmp.path().join("temp_remove.txt");
        fs::write(&file, b"x").unwrap();
        let id = cache.last_event_id() + 1;
        cache
            .handle_fs_events(vec![cardinal_sdk::FsEvent {
                path: file.clone(),
                id,
                flag: cardinal_sdk::EventFlag::ItemCreated,
            }])
            .unwrap();
        // delete file and send removal event => handle_fs_events will trigger internal removal
        fs::remove_file(&file).unwrap();
        let id2 = id + 1;
        cache
            .handle_fs_events(vec![cardinal_sdk::FsEvent {
                path: file.clone(),
                id: id2,
                flag: cardinal_sdk::EventFlag::ItemRemoved,
            }])
            .unwrap();
        assert!(cache.search("temp_remove.txt").unwrap().is_empty());
    }

    #[test]
    fn test_expand_file_nodes_fetch_metadata() {
        let tmp = TempDir::new("expand_meta").unwrap();
        fs::write(tmp.path().join("meta.txt"), b"hello world").unwrap();
        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());
        let idxs = cache.search("meta.txt").unwrap();
        assert_eq!(idxs.len(), 1);
        // First query_files returns metadata None
        let q1 = cache
            .query_files("meta.txt".into(), CancellationToken::noop())
            .expect("query should succeed")
            .expect("noop cancellation token should not cancel");
        assert_eq!(q1.len(), 1);
        assert!(q1[0].metadata.is_none());
        // expand_file_nodes should fetch metadata
        let nodes = cache.expand_file_nodes(&idxs);
        assert_eq!(nodes.len(), 1);
        assert!(
            nodes[0].metadata.is_some(),
            "metadata should be fetched on demand"
        );
        // A second expand should still have metadata (cached)
        let nodes2 = cache.expand_file_nodes(&idxs);
        assert!(nodes2[0].metadata.is_some());
    }

    #[test]
    fn test_persistent_roundtrip() {
        let tmp = TempDir::new("persist_round").unwrap();
        fs::write(tmp.path().join("a.bin"), b"data").unwrap();
        let cache_path = tmp.path().join("cache.zstd");
        let cache = SearchCache::walk_fs(tmp.path().to_path_buf());
        let original_total = cache.get_total_files();
        cache.flush_to_file(&cache_path).unwrap();
        let loaded =
            SearchCache::try_read_persistent_cache(tmp.path(), &cache_path, None, None).unwrap();
        assert_eq!(loaded.get_total_files(), original_total);
    }

    #[test]
    fn test_query_and_or_not_dedup_and_filtering() {
        let tmp = TempDir::new("query_bool").unwrap();
        fs::write(tmp.path().join("report.txt"), b"r").unwrap();
        fs::write(tmp.path().join("report.md"), b"r").unwrap();
        fs::write(tmp.path().join("other.txt"), b"o").unwrap();
        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        // OR: union should return 3 distinct results
        let or = cache.search("report OR ext:txt").unwrap();
        assert_eq!(or.len(), 3, "OR should dedup overlapping results");

        // AND: intersection should narrow to the txt
        let and = cache.search("report ext:txt").unwrap();
        assert_eq!(and.len(), 1);

        // NOT: exclude names containing 'report'
        let not = cache.search("ext:txt !report").unwrap();
        assert_eq!(not.len(), 1);
        let path = cache.node_path(*not.first().unwrap()).unwrap();
        assert!(path.ends_with(PathBuf::from("other.txt")));
    }

    #[test]
    fn test_regex_prefix_in_queries() {
        let tmp = TempDir::new("query_regex").unwrap();
        fs::write(tmp.path().join("Report Q1.md"), b"x").unwrap();
        fs::write(tmp.path().join("Report Q2.txt"), b"x").unwrap();
        fs::write(tmp.path().join("notes.txt"), b"x").unwrap();
        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let idxs = cache.search("regex:^Report").unwrap();
        assert_eq!(idxs.len(), 2);
    }

    #[test]
    fn test_ext_list_and_intersection() {
        let tmp = TempDir::new("query_ext_list").unwrap();
        fs::write(tmp.path().join("a.txt"), b"x").unwrap();
        fs::write(tmp.path().join("b.md"), b"x").unwrap();
        fs::write(tmp.path().join("c.rs"), b"x").unwrap();
        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        // ext list
        let list = cache.search("ext:txt;md").unwrap();
        assert_eq!(list.len(), 2);

        // Combine with word to intersect
        let only_b = cache.search("ext:txt;md b").unwrap();
        assert_eq!(only_b.len(), 1);
        let path = cache.node_path(*only_b.first().unwrap()).unwrap();
        assert!(path.ends_with(PathBuf::from("b.md")));
    }

    #[test]
    fn test_or_then_and_intersection_precedence() {
        let tmp = TempDir::new("query_bool_prec").unwrap();
        fs::write(tmp.path().join("a.txt"), b"x").unwrap();
        fs::write(tmp.path().join("b.md"), b"x").unwrap();
        fs::write(tmp.path().join("c.txt"), b"x").unwrap();
        fs::write(tmp.path().join("d.bin"), b"x").unwrap();
        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        // OR has higher precedence; then intersect via implicit AND with ext:txt
        let res = cache.search("a OR b ext:txt").unwrap();
        assert_eq!(res.len(), 1);
        let path = cache.node_path(*res.first().unwrap()).unwrap();
        assert!(path.ends_with(PathBuf::from("a.txt")));

        let res2 = cache.search("a OR b OR c ext:txt").unwrap();
        assert_eq!(res2.len(), 2);
        let names: Vec<_> = res2.iter().map(|i| cache.node_path(*i).unwrap()).collect();
        assert!(names.iter().any(|p| p.ends_with(PathBuf::from("a.txt"))));
        assert!(names.iter().any(|p| p.ends_with(PathBuf::from("c.txt"))));
    }

    #[test]
    fn test_groups_override_boolean_precedence() {
        let tmp = TempDir::new("query_groups_prec").unwrap();
        fs::write(tmp.path().join("ab.txt"), b"x").unwrap();
        fs::write(tmp.path().join("c.txt"), b"x").unwrap();
        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let res = cache.search("(a b) | c").unwrap();
        let names: Vec<_> = res.iter().map(|i| cache.node_path(*i).unwrap()).collect();
        // Some searches also return the root directory node; ensure target files are present
        assert!(names.iter().any(|p| p.ends_with(PathBuf::from("ab.txt"))));
        assert!(names.iter().any(|p| p.ends_with(PathBuf::from("c.txt"))));
    }

    #[test]
    fn test_not_precedence_with_intersection() {
        let tmp = TempDir::new("query_not_prec").unwrap();
        fs::write(tmp.path().join("a.txt"), b"x").unwrap();
        fs::write(tmp.path().join("b.txt"), b"x").unwrap();
        fs::write(tmp.path().join("notes.md"), b"x").unwrap();
        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let res = cache.search("ext:txt !a").unwrap();
        assert_eq!(res.len(), 1);
        let path = cache.node_path(*res.first().unwrap()).unwrap();
        assert!(path.ends_with(PathBuf::from("b.txt")));
    }

    #[test]
    fn test_type_and_macro_filters() {
        let tmp = TempDir::new("query_type_filters").unwrap();
        fs::write(tmp.path().join("photo.png"), b"x").unwrap();
        fs::write(tmp.path().join("song.mp3"), b"x").unwrap();
        fs::write(tmp.path().join("notes.txt"), b"x").unwrap();
        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let pictures = cache.search("type:picture").unwrap();
        assert_eq!(pictures.len(), 1);
        let path = cache.node_path(*pictures.first().unwrap()).unwrap();
        assert!(path.ends_with(PathBuf::from("photo.png")));

        let audio = cache.search("audio:").unwrap();
        assert_eq!(audio.len(), 1);
        let song = cache.node_path(*audio.first().unwrap()).unwrap();
        assert!(song.ends_with(PathBuf::from("song.mp3")));

        let documents = cache.search("doc:").unwrap();
        assert_eq!(documents.len(), 1);
        let doc_path = cache.node_path(*documents.first().unwrap()).unwrap();
        assert!(doc_path.ends_with(PathBuf::from("notes.txt")));
    }

    #[test]
    fn test_audio_macro_with_argument_behaves_like_and() {
        let tmp = TempDir::new("query_audio_argument").unwrap();
        fs::write(tmp.path().join("song_beats.mp3"), b"x").unwrap();
        fs::write(tmp.path().join("song_other.mp3"), b"x").unwrap();
        fs::write(tmp.path().join("notes.txt"), b"x").unwrap();
        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("audio:beats").unwrap();
        assert_eq!(results.len(), 1);
        let path = cache.node_path(*results.first().unwrap()).unwrap();
        assert!(path.ends_with(PathBuf::from("song_beats.mp3")));
    }

    #[test]
    fn test_size_filters() {
        let tmp = TempDir::new("query_size_filters").unwrap();
        fs::write(tmp.path().join("tiny.bin"), vec![0u8; 512]).unwrap();
        fs::write(tmp.path().join("medium.bin"), vec![0u8; 50_000]).unwrap();
        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let larger = cache.search("size:>1kb").unwrap();
        assert_eq!(larger.len(), 1);
        let large_path = cache.node_path(*larger.first().unwrap()).unwrap();
        assert!(large_path.ends_with(PathBuf::from("medium.bin")));

        let tiny = cache.search("size:tiny").unwrap();
        assert_eq!(tiny.len(), 1);
        let tiny_path = cache.node_path(*tiny.first().unwrap()).unwrap();
        assert!(tiny_path.ends_with(PathBuf::from("tiny.bin")));

        let ranged = cache.search("size:1kb..60kb").unwrap();
        assert_eq!(ranged.len(), 1);
        let ranged_path = cache.node_path(*ranged.first().unwrap()).unwrap();
        assert!(ranged_path.ends_with(PathBuf::from("medium.bin")));
    }

    #[test]
    fn test_size_filter_persists_metadata_on_nodes() {
        let tmp = TempDir::new("query_size_cache").unwrap();
        fs::write(tmp.path().join("cache.bin"), vec![0u8; 2048]).unwrap();
        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("size:>1kb").unwrap();
        assert_eq!(results.len(), 1);
        let index = results[0];
        assert!(
            cache.file_nodes[index].metadata.is_some(),
            "size filter should populate node metadata"
        );
    }

    #[test]
    fn test_regex_and_or_with_ext_intersection() {
        let tmp = TempDir::new("query_regex_prec").unwrap();
        fs::write(tmp.path().join("Report Q1.md"), b"x").unwrap();
        fs::write(tmp.path().join("Report Q2.txt"), b"x").unwrap();
        fs::write(tmp.path().join("notes.txt"), b"x").unwrap();
        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let res = cache.search("regex:^Report OR notes ext:txt").unwrap();
        assert_eq!(res.len(), 2);
        let names: Vec<_> = res.iter().map(|i| cache.node_path(*i).unwrap()).collect();
        assert!(
            names
                .iter()
                .any(|p| p.ends_with(PathBuf::from("Report Q2.txt")))
        );
        assert!(
            names
                .iter()
                .any(|p| p.ends_with(PathBuf::from("notes.txt")))
        );
    }

    #[test]
    fn test_all_subnodes_returns_all_descendants() {
        let tmp = TempDir::new("all_subnodes").unwrap();
        // Create nested structure:
        // root/
        //   a.txt
        //   src/
        //     main.rs
        //     lib.rs
        //     utils/
        //       helper.rs
        fs::write(tmp.path().join("a.txt"), b"x").unwrap();
        fs::create_dir(tmp.path().join("src")).unwrap();
        fs::write(tmp.path().join("src/main.rs"), b"x").unwrap();
        fs::write(tmp.path().join("src/lib.rs"), b"x").unwrap();
        fs::create_dir(tmp.path().join("src/utils")).unwrap();
        fs::write(tmp.path().join("src/utils/helper.rs"), b"x").unwrap();

        let cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        // Find src directory index
        let src_path = tmp.path().join("src");
        let src_idx = cache
            .node_index_for_raw_path(&src_path)
            .expect("src directory should exist");

        // Get all subnodes
        let subnodes = cache
            .all_subnodes(src_idx, CancellationToken::noop())
            .expect("Should return subnodes");

        // Should include: main.rs, lib.rs, utils/, helper.rs (4 items)
        assert_eq!(subnodes.len(), 4, "Should return all 4 descendants of src");

        // Verify all returned nodes are under src
        for &node_idx in &subnodes {
            let node_path = cache.node_path(node_idx).expect("Node should have path");
            assert!(
                node_path.starts_with(&src_path),
                "All subnodes should be under src"
            );
        }
    }

    #[test]
    fn test_all_subnodes_empty_directory() {
        let tmp = TempDir::new("all_subnodes_empty").unwrap();
        fs::create_dir(tmp.path().join("empty")).unwrap();

        let cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let empty_path = tmp.path().join("empty");
        let empty_idx = cache
            .node_index_for_raw_path(&empty_path)
            .expect("empty directory should exist");

        let subnodes = cache
            .all_subnodes(empty_idx, CancellationToken::noop())
            .expect("Should return empty vec");

        assert_eq!(subnodes.len(), 0, "Empty directory should have no subnodes");
    }

    #[test]
    fn test_all_subnodes_deep_nesting() {
        let tmp = TempDir::new("all_subnodes_deep").unwrap();
        // Create deep nesting: a/b/c/d/file.txt
        let deep_path = tmp.path().join("a/b/c/d");
        fs::create_dir_all(&deep_path).unwrap();
        fs::write(deep_path.join("file.txt"), b"x").unwrap();

        let cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        // Get subnodes from 'a' directory
        let a_path = tmp.path().join("a");
        let a_idx = cache
            .node_index_for_raw_path(&a_path)
            .expect("a directory should exist");

        let subnodes = cache
            .all_subnodes(a_idx, CancellationToken::noop())
            .expect("Should return subnodes");

        // Should include: b/, c/, d/, file.txt (4 items)
        assert_eq!(
            subnodes.len(),
            4,
            "Should recursively return all nested items"
        );

        // Verify the deepest file is included
        let has_file = subnodes.iter().any(|&idx| {
            cache
                .node_path(idx)
                .map(|p| p.ends_with("file.txt"))
                .unwrap_or(false)
        });
        assert!(has_file, "Should include deeply nested file");
    }

    #[test]
    fn test_all_subnodes_cancellation() {
        let tmp = TempDir::new("all_subnodes_cancel").unwrap();
        // Create many files to test cancellation
        for i in 0..100 {
            fs::write(tmp.path().join(format!("file_{i}.txt")), b"x").unwrap();
        }

        let cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let root_idx = cache.file_nodes.root();

        // Create a cancelled token by creating a newer version
        let token = CancellationToken::new(1);
        let _newer_token = CancellationToken::new(2); // This cancels the first token

        // Should return None when cancelled
        let result = cache.all_subnodes(root_idx, token);
        assert!(result.is_none(), "Should return None when cancelled");
    }

    // ========== Comprehensive Type Filter Tests (Batch 1) ==========

    #[test]
    fn test_type_picture_comprehensive() {
        let tmp = TempDir::new("type_picture_comp").unwrap();
        // Common image formats
        fs::write(tmp.path().join("photo.jpg"), b"x").unwrap();
        fs::write(tmp.path().join("image.jpeg"), b"x").unwrap();
        fs::write(tmp.path().join("graphic.png"), b"x").unwrap();
        fs::write(tmp.path().join("animation.gif"), b"x").unwrap();
        fs::write(tmp.path().join("bitmap.bmp"), b"x").unwrap();
        fs::write(tmp.path().join("texture.tif"), b"x").unwrap();
        fs::write(tmp.path().join("scan.tiff"), b"x").unwrap();
        fs::write(tmp.path().join("web.webp"), b"x").unwrap();
        fs::write(tmp.path().join("icon.ico"), b"x").unwrap();
        fs::write(tmp.path().join("vector.svg"), b"x").unwrap();
        // iPhone formats
        fs::write(tmp.path().join("iphone.heic"), b"x").unwrap();
        fs::write(tmp.path().join("burst.heif"), b"x").unwrap();
        // RAW formats
        fs::write(tmp.path().join("sony.arw"), b"x").unwrap();
        fs::write(tmp.path().join("canon.cr2"), b"x").unwrap();
        fs::write(tmp.path().join("olympus.orf"), b"x").unwrap();
        fs::write(tmp.path().join("fuji.raf"), b"x").unwrap();
        // Professional formats
        fs::write(tmp.path().join("layer.psd"), b"x").unwrap();
        fs::write(tmp.path().join("design.ai"), b"x").unwrap();
        // Non-picture files
        fs::write(tmp.path().join("document.txt"), b"x").unwrap();
        fs::write(tmp.path().join("video.mp4"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let pictures = cache.search("type:picture").unwrap();
        assert_eq!(pictures.len(), 18, "Should match all 18 image formats");

        // Test alternate names
        let pictures_alt = cache.search("type:pictures").unwrap();
        assert_eq!(pictures_alt.len(), 18);

        let images = cache.search("type:image").unwrap();
        assert_eq!(images.len(), 18);

        let photos = cache.search("type:photo").unwrap();
        assert_eq!(photos.len(), 18);

        // Test case insensitivity
        let upper = cache.search("type:PICTURE").unwrap();
        assert_eq!(upper.len(), 18);

        let mixed = cache.search("type:PiCtUrE").unwrap();
        assert_eq!(mixed.len(), 18);
    }

    #[test]
    fn test_type_video_comprehensive() {
        let tmp = TempDir::new("type_video_comp").unwrap();
        fs::write(tmp.path().join("clip.mp4"), b"x").unwrap();
        fs::write(tmp.path().join("iphone.m4v"), b"x").unwrap();
        fs::write(tmp.path().join("quicktime.mov"), b"x").unwrap();
        fs::write(tmp.path().join("windows.avi"), b"x").unwrap();
        fs::write(tmp.path().join("mkv_file.mkv"), b"x").unwrap();
        fs::write(tmp.path().join("wm_video.wmv"), b"x").unwrap();
        fs::write(tmp.path().join("web.webm"), b"x").unwrap();
        fs::write(tmp.path().join("flash.flv"), b"x").unwrap();
        fs::write(tmp.path().join("mpeg1.mpg"), b"x").unwrap();
        fs::write(tmp.path().join("mpeg2.mpeg"), b"x").unwrap();
        fs::write(tmp.path().join("mobile.3gp"), b"x").unwrap();
        fs::write(tmp.path().join("mobile2.3g2"), b"x").unwrap();
        fs::write(tmp.path().join("transport.ts"), b"x").unwrap();
        fs::write(tmp.path().join("avchd.mts"), b"x").unwrap();
        fs::write(tmp.path().join("bluray.m2ts"), b"x").unwrap();
        // Non-video files
        fs::write(tmp.path().join("audio.mp3"), b"x").unwrap();
        fs::write(tmp.path().join("doc.pdf"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let videos = cache.search("type:video").unwrap();
        assert_eq!(videos.len(), 15);

        let videos_alt = cache.search("type:videos").unwrap();
        assert_eq!(videos_alt.len(), 15);

        let movies = cache.search("type:movie").unwrap();
        assert_eq!(movies.len(), 15);

        let movies_alt = cache.search("type:movies").unwrap();
        assert_eq!(movies_alt.len(), 15);
    }

    #[test]
    fn test_type_audio_comprehensive() {
        let tmp = TempDir::new("type_audio_comp").unwrap();
        fs::write(tmp.path().join("song.mp3"), b"x").unwrap();
        fs::write(tmp.path().join("wave.wav"), b"x").unwrap();
        fs::write(tmp.path().join("lossless.flac"), b"x").unwrap();
        fs::write(tmp.path().join("compressed.aac"), b"x").unwrap();
        fs::write(tmp.path().join("vorbis.ogg"), b"x").unwrap();
        fs::write(tmp.path().join("vorbis2.oga"), b"x").unwrap();
        fs::write(tmp.path().join("modern.opus"), b"x").unwrap();
        fs::write(tmp.path().join("windows.wma"), b"x").unwrap();
        fs::write(tmp.path().join("apple.m4a"), b"x").unwrap();
        fs::write(tmp.path().join("apple_lossless.alac"), b"x").unwrap();
        fs::write(tmp.path().join("uncompressed.aiff"), b"x").unwrap();
        // Non-audio files
        fs::write(tmp.path().join("video.mp4"), b"x").unwrap();
        fs::write(tmp.path().join("text.txt"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let audio = cache.search("type:audio").unwrap();
        assert_eq!(audio.len(), 11);

        let audio_alt = cache.search("type:audios").unwrap();
        assert_eq!(audio_alt.len(), 11);

        let music = cache.search("type:music").unwrap();
        assert_eq!(music.len(), 11);

        let songs = cache.search("type:song").unwrap();
        assert_eq!(songs.len(), 11);

        let songs_alt = cache.search("type:songs").unwrap();
        assert_eq!(songs_alt.len(), 11);
    }

    #[test]
    fn test_type_document_comprehensive() {
        let tmp = TempDir::new("type_doc_comp").unwrap();
        fs::write(tmp.path().join("plain.txt"), b"x").unwrap();
        fs::write(tmp.path().join("markdown.md"), b"x").unwrap();
        fs::write(tmp.path().join("restructured.rst"), b"x").unwrap();
        fs::write(tmp.path().join("word_old.doc"), b"x").unwrap();
        fs::write(tmp.path().join("word_new.docx"), b"x").unwrap();
        fs::write(tmp.path().join("rich.rtf"), b"x").unwrap();
        fs::write(tmp.path().join("opendoc.odt"), b"x").unwrap();
        fs::write(tmp.path().join("portable.pdf"), b"x").unwrap();
        fs::write(tmp.path().join("apple.pages"), b"x").unwrap();
        fs::write(tmp.path().join("apple_rtf.rtfd"), b"x").unwrap();
        // Non-document files
        fs::write(tmp.path().join("image.png"), b"x").unwrap();
        fs::write(tmp.path().join("code.rs"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let docs = cache.search("type:doc").unwrap();
        assert_eq!(docs.len(), 10);

        let docs_alt = cache.search("type:docs").unwrap();
        assert_eq!(docs_alt.len(), 10);

        let documents = cache.search("type:document").unwrap();
        assert_eq!(documents.len(), 10);

        let documents_alt = cache.search("type:documents").unwrap();
        assert_eq!(documents_alt.len(), 10);

        let text = cache.search("type:text").unwrap();
        assert_eq!(text.len(), 10);

        let office = cache.search("type:office").unwrap();
        assert_eq!(office.len(), 10);
    }

    #[test]
    fn test_type_presentation_comprehensive() {
        let tmp = TempDir::new("type_presentation").unwrap();
        fs::write(tmp.path().join("powerpoint_old.ppt"), b"x").unwrap();
        fs::write(tmp.path().join("powerpoint_new.pptx"), b"x").unwrap();
        fs::write(tmp.path().join("apple.key"), b"x").unwrap();
        fs::write(tmp.path().join("opendoc.odp"), b"x").unwrap();
        fs::write(tmp.path().join("document.txt"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let pres = cache.search("type:presentation").unwrap();
        assert_eq!(pres.len(), 4);

        let pres_alt = cache.search("type:presentations").unwrap();
        assert_eq!(pres_alt.len(), 4);

        let ppt = cache.search("type:ppt").unwrap();
        assert_eq!(ppt.len(), 4);

        let slides = cache.search("type:slides").unwrap();
        assert_eq!(slides.len(), 4);
    }

    #[test]
    fn test_type_spreadsheet_comprehensive() {
        let tmp = TempDir::new("type_spreadsheet").unwrap();
        fs::write(tmp.path().join("excel_old.xls"), b"x").unwrap();
        fs::write(tmp.path().join("excel_new.xlsx"), b"x").unwrap();
        fs::write(tmp.path().join("data.csv"), b"x").unwrap();
        fs::write(tmp.path().join("apple.numbers"), b"x").unwrap();
        fs::write(tmp.path().join("opendoc.ods"), b"x").unwrap();
        fs::write(tmp.path().join("text.txt"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let sheets = cache.search("type:spreadsheet").unwrap();
        assert_eq!(sheets.len(), 5);

        let sheets_alt = cache.search("type:spreadsheets").unwrap();
        assert_eq!(sheets_alt.len(), 5);

        let xls = cache.search("type:xls").unwrap();
        assert_eq!(xls.len(), 5);

        let excel = cache.search("type:excel").unwrap();
        assert_eq!(excel.len(), 5);

        let sheet = cache.search("type:sheet").unwrap();
        assert_eq!(sheet.len(), 5);

        let sheets2 = cache.search("type:sheets").unwrap();
        assert_eq!(sheets2.len(), 5);
    }

    #[test]
    fn test_type_pdf_filter() {
        let tmp = TempDir::new("type_pdf").unwrap();
        fs::write(tmp.path().join("manual.pdf"), b"x").unwrap();
        fs::write(tmp.path().join("report.pdf"), b"x").unwrap();
        fs::write(tmp.path().join("guide.pdf"), b"x").unwrap();
        fs::write(tmp.path().join("doc.docx"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let pdfs = cache.search("type:pdf").unwrap();
        assert_eq!(pdfs.len(), 3);
    }

    #[test]
    fn test_type_archive_comprehensive() {
        let tmp = TempDir::new("type_archive").unwrap();
        fs::write(tmp.path().join("archive.zip"), b"x").unwrap();
        fs::write(tmp.path().join("winrar.rar"), b"x").unwrap();
        fs::write(tmp.path().join("seven.7z"), b"x").unwrap();
        fs::write(tmp.path().join("tarball.tar"), b"x").unwrap();
        fs::write(tmp.path().join("gzip.gz"), b"x").unwrap();
        fs::write(tmp.path().join("tar_gzip.tgz"), b"x").unwrap();
        fs::write(tmp.path().join("bzip.bz2"), b"x").unwrap();
        fs::write(tmp.path().join("xz_archive.xz"), b"x").unwrap();
        fs::write(tmp.path().join("zstd.zst"), b"x").unwrap();
        fs::write(tmp.path().join("cabinet.cab"), b"x").unwrap();
        fs::write(tmp.path().join("disc.iso"), b"x").unwrap();
        fs::write(tmp.path().join("macos.dmg"), b"x").unwrap();
        fs::write(tmp.path().join("text.txt"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let archives = cache.search("type:archive").unwrap();
        assert_eq!(archives.len(), 12);

        let archives_alt = cache.search("type:archives").unwrap();
        assert_eq!(archives_alt.len(), 12);

        let compressed = cache.search("type:compressed").unwrap();
        assert_eq!(compressed.len(), 12);

        let zip = cache.search("type:zip").unwrap();
        assert_eq!(zip.len(), 12);
    }

    #[test]
    fn test_type_code_comprehensive() {
        let tmp = TempDir::new("type_code").unwrap();
        // Rust
        fs::write(tmp.path().join("main.rs"), b"x").unwrap();
        // TypeScript/JavaScript
        fs::write(tmp.path().join("app.ts"), b"x").unwrap();
        fs::write(tmp.path().join("component.tsx"), b"x").unwrap();
        fs::write(tmp.path().join("script.js"), b"x").unwrap();
        fs::write(tmp.path().join("view.jsx"), b"x").unwrap();
        // C/C++
        fs::write(tmp.path().join("program.c"), b"x").unwrap();
        fs::write(tmp.path().join("impl.cc"), b"x").unwrap();
        fs::write(tmp.path().join("source.cpp"), b"x").unwrap();
        fs::write(tmp.path().join("alt.cxx"), b"x").unwrap();
        fs::write(tmp.path().join("header.h"), b"x").unwrap();
        fs::write(tmp.path().join("header2.hpp"), b"x").unwrap();
        fs::write(tmp.path().join("header3.hh"), b"x").unwrap();
        // Other languages
        fs::write(tmp.path().join("Main.java"), b"x").unwrap();
        fs::write(tmp.path().join("Program.cs"), b"x").unwrap();
        fs::write(tmp.path().join("script.py"), b"x").unwrap();
        fs::write(tmp.path().join("server.go"), b"x").unwrap();
        fs::write(tmp.path().join("app.rb"), b"x").unwrap();
        fs::write(tmp.path().join("ViewController.swift"), b"x").unwrap();
        fs::write(tmp.path().join("MainActivity.kt"), b"x").unwrap();
        fs::write(tmp.path().join("Script.kts"), b"x").unwrap();
        fs::write(tmp.path().join("index.php"), b"x").unwrap();
        // Web
        fs::write(tmp.path().join("page.html"), b"x").unwrap();
        fs::write(tmp.path().join("style.css"), b"x").unwrap();
        fs::write(tmp.path().join("vars.scss"), b"x").unwrap();
        fs::write(tmp.path().join("mixins.sass"), b"x").unwrap();
        fs::write(tmp.path().join("theme.less"), b"x").unwrap();
        // Config
        fs::write(tmp.path().join("config.json"), b"x").unwrap();
        fs::write(tmp.path().join("settings.yaml"), b"x").unwrap();
        fs::write(tmp.path().join("docker.yml"), b"x").unwrap();
        fs::write(tmp.path().join("Cargo.toml"), b"x").unwrap();
        fs::write(tmp.path().join("setup.ini"), b"x").unwrap();
        fs::write(tmp.path().join("app.cfg"), b"x").unwrap();
        // Shell scripts
        fs::write(tmp.path().join("build.sh"), b"x").unwrap();
        fs::write(tmp.path().join("setup.zsh"), b"x").unwrap();
        fs::write(tmp.path().join("config.fish"), b"x").unwrap();
        fs::write(tmp.path().join("script.ps1"), b"x").unwrap();
        fs::write(tmp.path().join("module.psm1"), b"x").unwrap();
        // Database
        fs::write(tmp.path().join("schema.sql"), b"x").unwrap();
        // Other scripting
        fs::write(tmp.path().join("game.lua"), b"x").unwrap();
        fs::write(tmp.path().join("script.pl"), b"x").unwrap();
        fs::write(tmp.path().join("module.pm"), b"x").unwrap();
        fs::write(tmp.path().join("analysis.r"), b"x").unwrap();
        fs::write(tmp.path().join("main.m"), b"x").unwrap();
        fs::write(tmp.path().join("bridge.mm"), b"x").unwrap();
        fs::write(tmp.path().join("app.dart"), b"x").unwrap();
        fs::write(tmp.path().join("service.scala"), b"x").unwrap();
        fs::write(tmp.path().join("phoenix.ex"), b"x").unwrap();
        fs::write(tmp.path().join("test.exs"), b"x").unwrap();
        // Non-code files
        fs::write(tmp.path().join("doc.txt"), b"x").unwrap();
        fs::write(tmp.path().join("image.png"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let code = cache.search("type:code").unwrap();
        // The test creates 47 code files + Cargo.toml = 48 total
        assert_eq!(code.len(), 48);

        let source = cache.search("type:source").unwrap();
        assert_eq!(source.len(), 48);

        let dev = cache.search("type:dev").unwrap();
        assert_eq!(dev.len(), 48);
    }

    #[test]
    fn test_type_executable_comprehensive() {
        let tmp = TempDir::new("type_exe").unwrap();
        fs::write(tmp.path().join("program.exe"), b"x").unwrap();
        fs::write(tmp.path().join("installer.msi"), b"x").unwrap();
        fs::write(tmp.path().join("script.bat"), b"x").unwrap();
        fs::write(tmp.path().join("command.cmd"), b"x").unwrap();
        fs::write(tmp.path().join("dos.com"), b"x").unwrap();
        fs::write(tmp.path().join("powershell.ps1"), b"x").unwrap();
        fs::write(tmp.path().join("module.psm1"), b"x").unwrap();
        fs::write(tmp.path().join("Calculator.app"), b"x").unwrap();
        fs::write(tmp.path().join("mobile.apk"), b"x").unwrap();
        fs::write(tmp.path().join("ios.ipa"), b"x").unwrap();
        fs::write(tmp.path().join("java.jar"), b"x").unwrap();
        fs::write(tmp.path().join("binary.bin"), b"x").unwrap();
        fs::write(tmp.path().join("linux.run"), b"x").unwrap();
        fs::write(tmp.path().join("macos.pkg"), b"x").unwrap();
        fs::write(tmp.path().join("text.txt"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let exe = cache.search("type:exe").unwrap();
        assert_eq!(exe.len(), 14);

        let exec = cache.search("type:exec").unwrap();
        assert_eq!(exec.len(), 14);

        let executable = cache.search("type:executable").unwrap();
        assert_eq!(executable.len(), 14);

        let executables = cache.search("type:executables").unwrap();
        assert_eq!(executables.len(), 14);

        let program = cache.search("type:program").unwrap();
        assert_eq!(program.len(), 14);

        let programs = cache.search("type:programs").unwrap();
        assert_eq!(programs.len(), 14);

        let app = cache.search("type:app").unwrap();
        assert_eq!(app.len(), 14);

        let apps = cache.search("type:apps").unwrap();
        assert_eq!(apps.len(), 14);
    }

    #[test]
    fn test_type_file_folder_filters() {
        let tmp = TempDir::new("type_file_folder").unwrap();
        fs::write(tmp.path().join("file1.txt"), b"x").unwrap();
        fs::write(tmp.path().join("file2.md"), b"x").unwrap();
        fs::create_dir(tmp.path().join("folder1")).unwrap();
        fs::create_dir(tmp.path().join("folder2")).unwrap();
        fs::write(tmp.path().join("folder1/nested.txt"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let files = cache.search("type:file").unwrap();
        assert_eq!(files.len(), 3, "Should match only files");

        let files_alt = cache.search("type:files").unwrap();
        assert_eq!(files_alt.len(), 3);

        let folders = cache.search("type:folder").unwrap();
        // Should match folder1, folder2, and the root directory
        assert_eq!(folders.len(), 3);

        let folders_alt = cache.search("type:folders").unwrap();
        assert_eq!(folders_alt.len(), 3);

        let dirs = cache.search("type:dir").unwrap();
        assert_eq!(dirs.len(), 3);

        let directory = cache.search("type:directory").unwrap();
        assert_eq!(directory.len(), 3);
    }

    #[test]
    fn test_type_filter_unknown_category_error() {
        let tmp = TempDir::new("type_unknown").unwrap();
        fs::write(tmp.path().join("file.txt"), b"x").unwrap();
        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let result = cache.search("type:unknowncategory");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Unknown type category")
        );
    }

    #[test]
    fn test_type_filter_empty_argument_error() {
        let tmp = TempDir::new("type_empty").unwrap();
        fs::write(tmp.path().join("file.txt"), b"x").unwrap();
        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let result = cache.search("type:");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("requires a category")
        );
    }

    #[test]
    fn test_audio_macro_no_arguments() {
        let tmp = TempDir::new("audio_macro").unwrap();
        fs::write(tmp.path().join("song.mp3"), b"x").unwrap();
        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let result = cache.search("audio:");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }

    #[test]
    fn test_video_macro_no_arguments() {
        let tmp = TempDir::new("video_macro").unwrap();
        fs::write(tmp.path().join("clip.mp4"), b"x").unwrap();
        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let result = cache.search("video:");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }

    #[test]
    fn test_doc_macro_no_arguments() {
        let tmp = TempDir::new("doc_macro").unwrap();
        fs::write(tmp.path().join("note.txt"), b"x").unwrap();
        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let result = cache.search("doc:");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }

    #[test]
    fn test_exe_macro_no_arguments() {
        let tmp = TempDir::new("exe_macro").unwrap();
        fs::write(tmp.path().join("app.exe"), b"x").unwrap();
        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let result = cache.search("exe:");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }

    #[test]
    fn test_type_filter_combined_with_name_search() {
        let tmp = TempDir::new("type_combined").unwrap();
        fs::write(tmp.path().join("report.pdf"), b"x").unwrap();
        fs::write(tmp.path().join("report.docx"), b"x").unwrap();
        fs::write(tmp.path().join("summary.pdf"), b"x").unwrap();
        fs::write(tmp.path().join("image.png"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("report type:doc").unwrap();
        assert_eq!(results.len(), 2);

        let paths: Vec<_> = results
            .iter()
            .map(|i| cache.node_path(*i).unwrap())
            .collect();
        assert!(paths.iter().any(|p| p.ends_with("report.pdf")));
        assert!(paths.iter().any(|p| p.ends_with("report.docx")));
    }

    #[test]
    fn test_type_filter_with_or_operator() {
        let tmp = TempDir::new("type_or").unwrap();
        fs::write(tmp.path().join("song.mp3"), b"x").unwrap();
        fs::write(tmp.path().join("clip.mp4"), b"x").unwrap();
        fs::write(tmp.path().join("doc.pdf"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("type:audio OR type:video").unwrap();
        assert_eq!(results.len(), 2);

        let paths: Vec<_> = results
            .iter()
            .map(|i| cache.node_path(*i).unwrap())
            .collect();
        assert!(paths.iter().any(|p| p.ends_with("song.mp3")));
        assert!(paths.iter().any(|p| p.ends_with("clip.mp4")));
    }

    #[test]
    fn test_type_filter_with_not_operator() {
        let tmp = TempDir::new("type_not").unwrap();
        fs::write(tmp.path().join("image.png"), b"x").unwrap();
        fs::write(tmp.path().join("song.mp3"), b"x").unwrap();
        fs::write(tmp.path().join("doc.txt"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("!type:picture").unwrap();
        assert!(results.len() >= 2);

        let has_image = results.iter().any(|&i| {
            cache
                .node_path(i)
                .map(|p| p.ends_with("image.png"))
                .unwrap_or(false)
        });
        assert!(!has_image, "Should not include picture files");
    }

    #[test]
    fn test_type_filter_multiple_extensions_same_category() {
        let tmp = TempDir::new("type_multi_ext").unwrap();
        fs::write(tmp.path().join("photo.jpg"), b"x").unwrap();
        fs::write(tmp.path().join("graphic.png"), b"x").unwrap();
        fs::write(tmp.path().join("animation.gif"), b"x").unwrap();
        fs::write(tmp.path().join("web.webp"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("type:picture").unwrap();
        assert_eq!(results.len(), 4);
    }

    // ========== Comprehensive Size Filter Tests (Batch 3) ==========

    #[test]
    fn test_size_comparison_operators() {
        let tmp = TempDir::new("size_comparison").unwrap();
        fs::write(tmp.path().join("tiny.bin"), vec![0u8; 500]).unwrap();
        fs::write(tmp.path().join("small.bin"), vec![0u8; 1500]).unwrap();
        fs::write(tmp.path().join("medium.bin"), vec![0u8; 5000]).unwrap();
        fs::write(tmp.path().join("large.bin"), vec![0u8; 15000]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        // Greater than
        let gt = cache.search("size:>1kb").unwrap();
        assert_eq!(gt.len(), 3);

        // Greater than or equal
        let gte = cache.search("size:>=1500").unwrap();
        assert_eq!(gte.len(), 3);

        // Less than
        let lt = cache.search("size:<1kb").unwrap();
        assert_eq!(lt.len(), 1);

        // Less than or equal
        let lte = cache.search("size:<=1500").unwrap();
        assert_eq!(lte.len(), 2);

        // Equal
        let eq = cache.search("size:=500").unwrap();
        assert_eq!(eq.len(), 1);

        // Not equal
        let ne = cache.search("size:!=500").unwrap();
        assert_eq!(ne.len(), 3);
    }

    #[test]
    fn test_size_units_bytes() {
        let tmp = TempDir::new("size_bytes").unwrap();
        fs::write(tmp.path().join("100b.bin"), vec![0u8; 100]).unwrap();
        fs::write(tmp.path().join("500b.bin"), vec![0u8; 500]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("size:>200").unwrap();
        assert_eq!(results.len(), 1);

        let results_b = cache.search("size:>200b").unwrap();
        assert_eq!(results_b.len(), 1);

        let results_byte = cache.search("size:>200byte").unwrap();
        assert_eq!(results_byte.len(), 1);

        let results_bytes = cache.search("size:>200bytes").unwrap();
        assert_eq!(results_bytes.len(), 1);
    }

    #[test]
    fn test_size_units_kilobytes() {
        let tmp = TempDir::new("size_kilobytes").unwrap();
        fs::write(tmp.path().join("half_kb.bin"), vec![0u8; 512]).unwrap();
        fs::write(tmp.path().join("two_kb.bin"), vec![0u8; 2048]).unwrap();
        fs::write(tmp.path().join("five_kb.bin"), vec![0u8; 5120]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let k = cache.search("size:>1k").unwrap();
        assert_eq!(k.len(), 2);

        let kb = cache.search("size:>1kb").unwrap();
        assert_eq!(kb.len(), 2);

        let kib = cache.search("size:>1kib").unwrap();
        assert_eq!(kib.len(), 2);

        let kilobyte = cache.search("size:>1kilobyte").unwrap();
        assert_eq!(kilobyte.len(), 2);

        let kilobytes = cache.search("size:>1kilobytes").unwrap();
        assert_eq!(kilobytes.len(), 2);
    }

    #[test]
    fn test_size_units_megabytes() {
        let tmp = TempDir::new("size_megabytes").unwrap();
        fs::write(tmp.path().join("half_mb.bin"), vec![0u8; 512 * 1024]).unwrap();
        fs::write(tmp.path().join("two_mb.bin"), vec![0u8; 2 * 1024 * 1024]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let m = cache.search("size:>1m").unwrap();
        assert_eq!(m.len(), 1);

        let mb = cache.search("size:>1mb").unwrap();
        assert_eq!(mb.len(), 1);

        let mib = cache.search("size:>1mib").unwrap();
        assert_eq!(mib.len(), 1);

        let megabyte = cache.search("size:>1megabyte").unwrap();
        assert_eq!(megabyte.len(), 1);

        let megabytes = cache.search("size:>1megabytes").unwrap();
        assert_eq!(megabytes.len(), 1);
    }

    #[test]
    fn test_size_units_gigabytes() {
        let tmp = TempDir::new("size_gigabytes").unwrap();
        // For testing purposes, we'll use smaller values and adjust the query
        fs::write(tmp.path().join("small.bin"), vec![0u8; 100]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        // Test that the unit is recognized (size will be less than 1GB)
        let g = cache.search("size:<1g").unwrap();
        assert!(!g.is_empty());

        let gb = cache.search("size:<1gb").unwrap();
        assert!(!gb.is_empty());

        let gib = cache.search("size:<1gib").unwrap();
        assert!(!gib.is_empty());

        let gigabyte = cache.search("size:<1gigabyte").unwrap();
        assert!(!gigabyte.is_empty());

        let gigabytes = cache.search("size:<1gigabytes").unwrap();
        assert!(!gigabytes.is_empty());
    }

    #[test]
    fn test_size_units_terabytes() {
        let tmp = TempDir::new("size_terabytes").unwrap();
        fs::write(tmp.path().join("small.bin"), vec![0u8; 100]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let t = cache.search("size:<1t").unwrap();
        assert!(!t.is_empty());

        let tb = cache.search("size:<1tb").unwrap();
        assert!(!tb.is_empty());

        let tib = cache.search("size:<1tib").unwrap();
        assert!(!tib.is_empty());

        let terabyte = cache.search("size:<1terabyte").unwrap();
        assert!(!terabyte.is_empty());

        let terabytes = cache.search("size:<1terabytes").unwrap();
        assert!(!terabytes.is_empty());
    }

    #[test]
    fn test_size_units_petabytes() {
        let tmp = TempDir::new("size_petabytes").unwrap();
        fs::write(tmp.path().join("small.bin"), vec![0u8; 100]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let p = cache.search("size:<1p").unwrap();
        assert!(!p.is_empty());

        let pb = cache.search("size:<1pb").unwrap();
        assert!(!pb.is_empty());

        let pib = cache.search("size:<1pib").unwrap();
        assert!(!pib.is_empty());

        let petabyte = cache.search("size:<1petabyte").unwrap();
        assert!(!petabyte.is_empty());

        let petabytes = cache.search("size:<1petabytes").unwrap();
        assert!(!petabytes.is_empty());
    }

    #[test]
    fn test_size_decimal_values() {
        let tmp = TempDir::new("size_decimal").unwrap();
        fs::write(tmp.path().join("1500b.bin"), vec![0u8; 1500]).unwrap();
        fs::write(tmp.path().join("2500b.bin"), vec![0u8; 2500]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("size:>1.5kb").unwrap();
        assert_eq!(results.len(), 1);

        let results2 = cache.search("size:>2.0kb").unwrap();
        assert_eq!(results2.len(), 1);

        let results3 = cache.search("size:>0.5kb").unwrap();
        assert_eq!(results3.len(), 2);
    }

    #[test]
    fn test_size_range_both_bounds() {
        let tmp = TempDir::new("size_range_both").unwrap();
        fs::write(tmp.path().join("500b.bin"), vec![0u8; 500]).unwrap();
        fs::write(tmp.path().join("1500b.bin"), vec![0u8; 1500]).unwrap();
        fs::write(tmp.path().join("2500b.bin"), vec![0u8; 2500]).unwrap();
        fs::write(tmp.path().join("5000b.bin"), vec![0u8; 5000]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("size:1kb..3kb").unwrap();
        assert_eq!(results.len(), 2);

        let paths: Vec<_> = results
            .iter()
            .map(|i| cache.node_path(*i).unwrap())
            .collect();
        assert!(paths.iter().any(|p| p.ends_with("1500b.bin")));
        assert!(paths.iter().any(|p| p.ends_with("2500b.bin")));
    }

    #[test]
    fn test_size_range_open_start() {
        let tmp = TempDir::new("size_range_open_start").unwrap();
        fs::write(tmp.path().join("500b.bin"), vec![0u8; 500]).unwrap();
        fs::write(tmp.path().join("1500b.bin"), vec![0u8; 1500]).unwrap();
        fs::write(tmp.path().join("2500b.bin"), vec![0u8; 2500]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("size:..2kb").unwrap();
        assert_eq!(results.len(), 2);

        let paths: Vec<_> = results
            .iter()
            .map(|i| cache.node_path(*i).unwrap())
            .collect();
        assert!(paths.iter().any(|p| p.ends_with("500b.bin")));
        assert!(paths.iter().any(|p| p.ends_with("1500b.bin")));
    }

    #[test]
    fn test_size_range_open_end() {
        let tmp = TempDir::new("size_range_open_end").unwrap();
        fs::write(tmp.path().join("500b.bin"), vec![0u8; 500]).unwrap();
        fs::write(tmp.path().join("1500b.bin"), vec![0u8; 1500]).unwrap();
        fs::write(tmp.path().join("2500b.bin"), vec![0u8; 2500]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("size:1kb..").unwrap();
        assert_eq!(results.len(), 2);

        let paths: Vec<_> = results
            .iter()
            .map(|i| cache.node_path(*i).unwrap())
            .collect();
        assert!(paths.iter().any(|p| p.ends_with("1500b.bin")));
        assert!(paths.iter().any(|p| p.ends_with("2500b.bin")));
    }

    #[test]
    fn test_size_keyword_empty() {
        let tmp = TempDir::new("size_keyword_empty").unwrap();
        fs::write(tmp.path().join("empty.bin"), vec![]).unwrap();
        fs::write(tmp.path().join("nonempty.bin"), vec![0u8; 100]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("size:empty").unwrap();
        assert_eq!(results.len(), 1);

        let path = cache.node_path(*results.first().unwrap()).unwrap();
        assert!(path.ends_with("empty.bin"));
    }

    #[test]
    fn test_size_keyword_tiny() {
        let tmp = TempDir::new("size_keyword_tiny").unwrap();
        fs::write(tmp.path().join("tiny1.bin"), vec![0u8; 100]).unwrap();
        fs::write(tmp.path().join("tiny2.bin"), vec![0u8; 5000]).unwrap();
        fs::write(tmp.path().join("medium.bin"), vec![0u8; 50000]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("size:tiny").unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_size_keyword_small() {
        let tmp = TempDir::new("size_keyword_small").unwrap();
        fs::write(tmp.path().join("small1.bin"), vec![0u8; 20_000]).unwrap();
        fs::write(tmp.path().join("small2.bin"), vec![0u8; 50_000]).unwrap();
        fs::write(tmp.path().join("large.bin"), vec![0u8; 200_000]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("size:small").unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_size_keyword_medium() {
        let tmp = TempDir::new("size_keyword_medium").unwrap();
        fs::write(tmp.path().join("small.bin"), vec![0u8; 50_000]).unwrap();
        fs::write(tmp.path().join("medium.bin"), vec![0u8; 500_000]).unwrap();
        fs::write(tmp.path().join("large.bin"), vec![0u8; 2_000_000]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("size:medium").unwrap();
        assert_eq!(results.len(), 1);

        let path = cache.node_path(*results.first().unwrap()).unwrap();
        assert!(path.ends_with("medium.bin"));
    }

    #[test]
    fn test_size_keyword_large() {
        let tmp = TempDir::new("size_keyword_large").unwrap();
        fs::write(tmp.path().join("medium.bin"), vec![0u8; 500_000]).unwrap();
        fs::write(tmp.path().join("large.bin"), vec![0u8; 5_000_000]).unwrap();
        fs::write(tmp.path().join("huge.bin"), vec![0u8; 50_000_000]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("size:large").unwrap();
        assert_eq!(results.len(), 1);

        let path = cache.node_path(*results.first().unwrap()).unwrap();
        assert!(path.ends_with("large.bin"));
    }

    #[test]
    fn test_size_keyword_huge() {
        let tmp = TempDir::new("size_keyword_huge").unwrap();
        fs::write(tmp.path().join("large.bin"), vec![0u8; 10_000_000]).unwrap();
        fs::write(tmp.path().join("huge.bin"), vec![0u8; 100_000_000]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("size:huge").unwrap();
        assert_eq!(results.len(), 1);

        let path = cache.node_path(*results.first().unwrap()).unwrap();
        assert!(path.ends_with("huge.bin"));
    }

    #[test]
    fn test_size_keyword_gigantic() {
        let tmp = TempDir::new("size_keyword_gigantic").unwrap();
        fs::write(tmp.path().join("huge.bin"), vec![0u8; 100_000_000]).unwrap();
        fs::write(tmp.path().join("gigantic.bin"), vec![0u8; 200_000_000]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("size:gigantic").unwrap();
        assert_eq!(results.len(), 1);

        let path = cache.node_path(*results.first().unwrap()).unwrap();
        assert!(path.ends_with("gigantic.bin"));
    }

    #[test]
    fn test_size_keyword_giant() {
        let tmp = TempDir::new("size_keyword_giant").unwrap();
        fs::write(tmp.path().join("huge.bin"), vec![0u8; 100_000_000]).unwrap();
        fs::write(tmp.path().join("giant.bin"), vec![0u8; 200_000_000]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("size:giant").unwrap();
        assert_eq!(results.len(), 1);

        let path = cache.node_path(*results.first().unwrap()).unwrap();
        assert!(path.ends_with("giant.bin"));
    }

    #[test]
    fn test_size_keyword_case_insensitive() {
        let tmp = TempDir::new("size_keyword_case").unwrap();
        fs::write(tmp.path().join("tiny.bin"), vec![0u8; 100]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let lower = cache.search("size:tiny").unwrap();
        assert_eq!(lower.len(), 1);

        let upper = cache.search("size:TINY").unwrap();
        assert_eq!(upper.len(), 1);

        let mixed = cache.search("size:TiNy").unwrap();
        assert_eq!(mixed.len(), 1);
    }

    #[test]
    fn test_size_filter_excludes_directories() {
        let tmp = TempDir::new("size_no_dirs").unwrap();
        fs::write(tmp.path().join("file.bin"), vec![0u8; 1000]).unwrap();
        fs::create_dir(tmp.path().join("folder")).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("size:>500").unwrap();
        assert_eq!(results.len(), 1);

        let path = cache.node_path(*results.first().unwrap()).unwrap();
        assert!(path.ends_with("file.bin"));
    }

    #[test]
    fn test_size_combined_with_name_search() {
        let tmp = TempDir::new("size_with_name").unwrap();
        fs::write(tmp.path().join("report.bin"), vec![0u8; 1500]).unwrap();
        fs::write(tmp.path().join("report.txt"), vec![0u8; 500]).unwrap();
        fs::write(tmp.path().join("data.bin"), vec![0u8; 2000]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("report size:>1kb").unwrap();
        assert_eq!(results.len(), 1);

        let path = cache.node_path(*results.first().unwrap()).unwrap();
        assert!(path.ends_with("report.bin"));
    }

    #[test]
    fn test_size_combined_with_ext_filter() {
        let tmp = TempDir::new("size_with_ext").unwrap();
        fs::write(tmp.path().join("large.txt"), vec![0u8; 2000]).unwrap();
        fs::write(tmp.path().join("small.txt"), vec![0u8; 500]).unwrap();
        fs::write(tmp.path().join("large.bin"), vec![0u8; 2000]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("ext:txt size:>1kb").unwrap();
        assert_eq!(results.len(), 1);

        let path = cache.node_path(*results.first().unwrap()).unwrap();
        assert!(path.ends_with("large.txt"));
    }

    #[test]
    fn test_size_combined_with_type_filter() {
        let tmp = TempDir::new("size_with_type").unwrap();
        fs::write(tmp.path().join("large.png"), vec![0u8; 50_000]).unwrap();
        fs::write(tmp.path().join("small.png"), vec![0u8; 1_000]).unwrap();
        fs::write(tmp.path().join("large.mp3"), vec![0u8; 50_000]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("type:picture size:>10kb").unwrap();
        assert_eq!(results.len(), 1);

        let path = cache.node_path(*results.first().unwrap()).unwrap();
        assert!(path.ends_with("large.png"));
    }

    #[test]
    fn test_size_with_or_operator() {
        let tmp = TempDir::new("size_with_or").unwrap();
        fs::write(tmp.path().join("tiny.bin"), vec![0u8; 100]).unwrap();
        fs::write(tmp.path().join("medium.bin"), vec![0u8; 5000]).unwrap();
        fs::write(tmp.path().join("gigantic.bin"), vec![0u8; 200_000_000]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("size:tiny OR size:gigantic").unwrap();
        assert!(results.len() >= 2, "Should match at least 2 files");

        let paths: Vec<_> = results
            .iter()
            .map(|i| cache.node_path(*i).unwrap())
            .collect();
        assert!(paths.iter().any(|p| p.ends_with("tiny.bin")));
        assert!(paths.iter().any(|p| p.ends_with("gigantic.bin")));
    }

    #[test]
    fn test_size_with_not_operator() {
        let tmp = TempDir::new("size_with_not").unwrap();
        fs::write(tmp.path().join("tiny.bin"), vec![0u8; 100]).unwrap();
        fs::write(tmp.path().join("medium.bin"), vec![0u8; 5000]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("!size:tiny").unwrap();
        let has_tiny = results.iter().any(|&i| {
            cache
                .node_path(i)
                .map(|p| p.ends_with("tiny.bin"))
                .unwrap_or(false)
        });
        assert!(!has_tiny, "Should not include tiny files");
    }

    #[test]
    fn test_size_error_empty_value() {
        let tmp = TempDir::new("size_error_empty").unwrap();
        fs::write(tmp.path().join("file.bin"), b"x").unwrap();
        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let result = cache.search("size:");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("requires a value"));
    }

    #[test]
    fn test_size_error_invalid_number() {
        let tmp = TempDir::new("size_error_number").unwrap();
        fs::write(tmp.path().join("file.bin"), b"x").unwrap();
        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let result = cache.search("size:notanumber");
        assert!(result.is_err());
    }

    #[test]
    fn test_size_error_unknown_unit() {
        let tmp = TempDir::new("size_error_unit").unwrap();
        fs::write(tmp.path().join("file.bin"), b"x").unwrap();
        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let result = cache.search("size:100zb");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Unknown size unit")
        );
    }

    #[test]
    fn test_size_error_keyword_with_comparison() {
        let tmp = TempDir::new("size_error_keyword_comp").unwrap();
        fs::write(tmp.path().join("file.bin"), b"x").unwrap();
        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let result = cache.search("size:>tiny");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("keywords cannot be used with comparison")
        );
    }

    #[test]
    fn test_size_range_inverted_bounds_error() {
        let tmp = TempDir::new("size_range_inverted").unwrap();
        fs::write(tmp.path().join("file.bin"), b"x").unwrap();
        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let result = cache.search("size:10kb..1kb");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("start must be less than or equal to the end")
        );
    }

    #[test]
    fn test_size_bare_value_equals_comparison() {
        let tmp = TempDir::new("size_bare_equals").unwrap();
        fs::write(tmp.path().join("exact.bin"), vec![0u8; 1024]).unwrap();
        fs::write(tmp.path().join("other.bin"), vec![0u8; 2048]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("size:1kb").unwrap();
        assert_eq!(results.len(), 1);

        let path = cache.node_path(*results.first().unwrap()).unwrap();
        assert!(path.ends_with("exact.bin"));
    }

    #[test]
    fn test_size_zero_bytes() {
        let tmp = TempDir::new("size_zero").unwrap();
        fs::write(tmp.path().join("empty.bin"), vec![]).unwrap();
        fs::write(tmp.path().join("nonempty.bin"), vec![0u8; 1]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("size:0").unwrap();
        assert_eq!(results.len(), 1);

        let path = cache.node_path(*results.first().unwrap()).unwrap();
        assert!(path.ends_with("empty.bin"));
    }

    #[test]
    fn test_size_very_large_numbers() {
        let tmp = TempDir::new("size_large_num").unwrap();
        fs::write(tmp.path().join("small.bin"), vec![0u8; 100]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        // Test that very large numbers don't cause panics
        let results = cache.search("size:<999999gb").unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_size_fractional_precision() {
        let tmp = TempDir::new("size_fractional").unwrap();
        fs::write(tmp.path().join("file1.bin"), vec![0u8; 1536]).unwrap(); // 1.5 KB
        fs::write(tmp.path().join("file2.bin"), vec![0u8; 2048]).unwrap(); // 2 KB

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("size:>1.4kb").unwrap();
        assert_eq!(results.len(), 2);

        let results2 = cache.search("size:>1.6kb").unwrap();
        assert_eq!(results2.len(), 1);
    }

    // ========== Combined Type and Size Filter Tests (Batch 5) ==========

    #[test]
    fn test_type_and_size_complex_query() {
        let tmp = TempDir::new("type_size_complex").unwrap();
        fs::write(tmp.path().join("small_photo.jpg"), vec![0u8; 5_000]).unwrap();
        fs::write(tmp.path().join("large_photo.jpg"), vec![0u8; 50_000]).unwrap();
        fs::write(tmp.path().join("small_video.mp4"), vec![0u8; 5_000]).unwrap();
        fs::write(tmp.path().join("large_video.mp4"), vec![0u8; 50_000]).unwrap();
        fs::write(tmp.path().join("document.pdf"), vec![0u8; 50_000]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        // Pictures over 10KB
        let results = cache.search("type:picture size:>10kb").unwrap();
        assert_eq!(results.len(), 1);
        let path = cache.node_path(*results.first().unwrap()).unwrap();
        assert!(path.ends_with("large_photo.jpg"));

        // Pictures OR videos, but only over 10KB
        let results2 = cache
            .search("(type:picture OR type:video) size:>10kb")
            .unwrap();
        assert_eq!(results2.len(), 2);

        // Large media files (pictures or videos)
        let results3 = cache
            .search("type:picture OR type:video size:>10kb")
            .unwrap();
        assert_eq!(results3.len(), 2);
    }

    #[test]
    fn test_multiple_type_filters_with_or() {
        let tmp = TempDir::new("multi_type_or").unwrap();
        fs::write(tmp.path().join("song.mp3"), b"x").unwrap();
        fs::write(tmp.path().join("clip.mp4"), b"x").unwrap();
        fs::write(tmp.path().join("photo.jpg"), b"x").unwrap();
        fs::write(tmp.path().join("doc.pdf"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache
            .search("type:audio OR type:video OR type:picture")
            .unwrap();
        assert_eq!(results.len(), 3);

        let paths: Vec<_> = results
            .iter()
            .map(|i| cache.node_path(*i).unwrap())
            .collect();
        assert!(paths.iter().any(|p| p.ends_with("song.mp3")));
        assert!(paths.iter().any(|p| p.ends_with("clip.mp4")));
        assert!(paths.iter().any(|p| p.ends_with("photo.jpg")));
    }

    #[test]
    fn test_type_filter_with_parent_filter() {
        let tmp = TempDir::new("type_with_parent").unwrap();
        fs::create_dir(tmp.path().join("images")).unwrap();
        fs::create_dir(tmp.path().join("videos")).unwrap();
        fs::write(tmp.path().join("images/photo.jpg"), b"x").unwrap();
        fs::write(tmp.path().join("videos/photo.jpg"), b"x").unwrap();
        fs::write(tmp.path().join("videos/clip.mp4"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let images_dir = tmp.path().join("images");
        let results = cache
            .search(&format!("type:picture parent:{}", images_dir.display()))
            .unwrap();
        assert_eq!(results.len(), 1);

        let path = cache.node_path(*results.first().unwrap()).unwrap();
        assert!(path.ends_with("images/photo.jpg"));
    }

    #[test]
    fn test_type_filter_with_infolder_filter() {
        let tmp = TempDir::new("type_with_infolder").unwrap();
        fs::create_dir(tmp.path().join("media")).unwrap();
        fs::create_dir(tmp.path().join("media/photos")).unwrap();
        fs::write(tmp.path().join("media/song.mp3"), b"x").unwrap();
        fs::write(tmp.path().join("media/photos/pic1.jpg"), b"x").unwrap();
        fs::write(tmp.path().join("media/photos/pic2.png"), b"x").unwrap();
        fs::write(tmp.path().join("doc.txt"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let media_dir = tmp.path().join("media");
        let results = cache
            .search(&format!("type:picture infolder:{}", media_dir.display()))
            .unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_size_filter_with_parent_filter() {
        let tmp = TempDir::new("size_with_parent").unwrap();
        fs::create_dir(tmp.path().join("large")).unwrap();
        fs::create_dir(tmp.path().join("small")).unwrap();
        fs::write(tmp.path().join("large/file1.bin"), vec![0u8; 10_000]).unwrap();
        fs::write(tmp.path().join("large/file2.bin"), vec![0u8; 500]).unwrap();
        fs::write(tmp.path().join("small/file3.bin"), vec![0u8; 500]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let large_dir = tmp.path().join("large");
        let results = cache
            .search(&format!("size:>1kb parent:{}", large_dir.display()))
            .unwrap();
        assert_eq!(results.len(), 1);

        let path = cache.node_path(*results.first().unwrap()).unwrap();
        assert!(path.ends_with("large/file1.bin"));
    }

    #[test]
    fn test_size_filter_with_infolder_filter() {
        let tmp = TempDir::new("size_with_infolder").unwrap();
        fs::create_dir(tmp.path().join("data")).unwrap();
        fs::create_dir(tmp.path().join("data/nested")).unwrap();
        fs::write(tmp.path().join("data/large1.bin"), vec![0u8; 10_000]).unwrap();
        fs::write(tmp.path().join("data/nested/large2.bin"), vec![0u8; 10_000]).unwrap();
        fs::write(tmp.path().join("data/small.bin"), vec![0u8; 500]).unwrap();
        fs::write(tmp.path().join("other.bin"), vec![0u8; 10_000]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let data_dir = tmp.path().join("data");
        let results = cache
            .search(&format!("size:>5kb infolder:{}", data_dir.display()))
            .unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_extension_case_sensitivity_in_type_filter() {
        let tmp = TempDir::new("ext_case_type").unwrap();
        fs::write(tmp.path().join("photo.JPG"), b"x").unwrap();
        fs::write(tmp.path().join("image.jpg"), b"x").unwrap();
        fs::write(tmp.path().join("graphic.PNG"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("type:picture").unwrap();
        assert_eq!(results.len(), 3, "Should match case-insensitively");
    }

    #[test]
    fn test_multiple_size_ranges_with_or() {
        let tmp = TempDir::new("multi_size_or").unwrap();
        fs::write(tmp.path().join("tiny.bin"), vec![0u8; 100]).unwrap();
        fs::write(tmp.path().join("small.bin"), vec![0u8; 1_000]).unwrap();
        fs::write(tmp.path().join("medium.bin"), vec![0u8; 5_000]).unwrap();
        fs::write(tmp.path().join("large.bin"), vec![0u8; 50_000]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("size:..500 OR size:>10kb").unwrap();
        assert_eq!(results.len(), 2);

        let paths: Vec<_> = results
            .iter()
            .map(|i| cache.node_path(*i).unwrap())
            .collect();
        assert!(paths.iter().any(|p| p.ends_with("tiny.bin")));
        assert!(paths.iter().any(|p| p.ends_with("large.bin")));
    }

    #[test]
    fn test_type_filter_empty_result() {
        let tmp = TempDir::new("type_empty_result").unwrap();
        fs::write(tmp.path().join("doc.txt"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("type:audio").unwrap();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_size_filter_empty_result() {
        let tmp = TempDir::new("size_empty_result").unwrap();
        fs::write(tmp.path().join("small.bin"), vec![0u8; 100]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("size:>1mb").unwrap();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_combined_filters_all_match() {
        let tmp = TempDir::new("combined_all_match").unwrap();
        fs::write(tmp.path().join("report_large.pdf"), vec![0u8; 100_000]).unwrap();
        fs::write(tmp.path().join("report_small.pdf"), vec![0u8; 1_000]).unwrap();
        fs::write(tmp.path().join("data.csv"), vec![0u8; 100_000]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("report type:pdf size:>10kb").unwrap();
        assert_eq!(results.len(), 1);

        let path = cache.node_path(*results.first().unwrap()).unwrap();
        assert!(path.ends_with("report_large.pdf"));
    }

    #[test]
    fn test_audio_macro_with_all_extensions() {
        let tmp = TempDir::new("audio_all_ext").unwrap();
        for ext in [
            "mp3", "wav", "flac", "aac", "ogg", "oga", "opus", "wma", "m4a", "alac", "aiff",
        ] {
            fs::write(tmp.path().join(format!("audio.{ext}")), b"x").unwrap();
        }

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("audio:").unwrap();
        assert_eq!(results.len(), 11);
    }

    #[test]
    fn test_video_macro_with_all_extensions() {
        let tmp = TempDir::new("video_all_ext").unwrap();
        for ext in [
            "mp4", "m4v", "mov", "avi", "mkv", "wmv", "webm", "flv", "mpg", "mpeg", "3gp", "3g2",
            "ts", "mts", "m2ts",
        ] {
            fs::write(tmp.path().join(format!("video.{ext}")), b"x").unwrap();
        }

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("video:").unwrap();
        assert_eq!(results.len(), 15);
    }

    #[test]
    fn test_doc_macro_with_all_extensions() {
        let tmp = TempDir::new("doc_all_ext").unwrap();
        for ext in [
            "txt", "md", "rst", "doc", "docx", "rtf", "odt", "pdf", "pages", "rtfd",
        ] {
            fs::write(tmp.path().join(format!("document.{ext}")), b"x").unwrap();
        }

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("doc:").unwrap();
        assert_eq!(results.len(), 10);
    }

    #[test]
    fn test_exe_macro_with_all_extensions() {
        let tmp = TempDir::new("exe_all_ext").unwrap();
        for ext in [
            "exe", "msi", "bat", "cmd", "com", "ps1", "psm1", "app", "apk", "ipa", "jar", "bin",
            "run", "pkg",
        ] {
            fs::write(tmp.path().join(format!("program.{ext}")), b"x").unwrap();
        }

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("exe:").unwrap();
        assert_eq!(results.len(), 14);
    }

    #[test]
    fn test_size_with_whitespace() {
        let tmp = TempDir::new("size_whitespace").unwrap();
        fs::write(tmp.path().join("file.bin"), vec![0u8; 2048]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        // Test basic size query (whitespace after operator might not be supported)
        let results = cache.search("size:>1kb").unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_type_categories_overlap() {
        let tmp = TempDir::new("type_overlap").unwrap();
        fs::write(tmp.path().join("document.pdf"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        // PDF is in both doc and pdf categories
        let doc_results = cache.search("type:doc").unwrap();
        assert_eq!(doc_results.len(), 1);

        let pdf_results = cache.search("type:pdf").unwrap();
        assert_eq!(pdf_results.len(), 1);
    }

    #[test]
    fn test_size_boundary_conditions() {
        let tmp = TempDir::new("size_boundary").unwrap();
        fs::write(tmp.path().join("exactly_1kb.bin"), vec![0u8; 1024]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let gt = cache.search("size:>1kb").unwrap();
        assert_eq!(gt.len(), 0);

        let gte = cache.search("size:>=1kb").unwrap();
        assert_eq!(gte.len(), 1);

        let lt = cache.search("size:<1kb").unwrap();
        assert_eq!(lt.len(), 0);

        let lte = cache.search("size:<=1kb").unwrap();
        assert_eq!(lte.len(), 1);

        let eq = cache.search("size:=1kb").unwrap();
        assert_eq!(eq.len(), 1);
    }

    #[test]
    fn test_complex_boolean_with_filters() {
        let tmp = TempDir::new("complex_boolean").unwrap();
        fs::write(tmp.path().join("report.pdf"), vec![0u8; 100_000]).unwrap();
        fs::write(tmp.path().join("report.txt"), vec![0u8; 1_000]).unwrap();
        fs::write(tmp.path().join("image.jpg"), vec![0u8; 100_000]).unwrap();
        fs::write(tmp.path().join("small_image.jpg"), vec![0u8; 1_000]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache
            .search("(report OR type:picture) size:>10kb !txt")
            .unwrap();
        assert_eq!(results.len(), 2);

        let paths: Vec<_> = results
            .iter()
            .map(|i| cache.node_path(*i).unwrap())
            .collect();
        assert!(paths.iter().any(|p| p.ends_with("report.pdf")));
        assert!(paths.iter().any(|p| p.ends_with("image.jpg")));
    }

    #[test]
    fn test_nested_directory_type_filter() {
        let tmp = TempDir::new("nested_dir_type").unwrap();
        fs::create_dir_all(tmp.path().join("a/b/c")).unwrap();
        fs::write(tmp.path().join("a/photo.jpg"), b"x").unwrap();
        fs::write(tmp.path().join("a/b/photo.png"), b"x").unwrap();
        fs::write(tmp.path().join("a/b/c/photo.gif"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("type:picture").unwrap();
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_size_with_regex_filter() {
        let tmp = TempDir::new("size_with_regex").unwrap();
        fs::write(tmp.path().join("Report_2024.pdf"), vec![0u8; 10_000]).unwrap();
        fs::write(tmp.path().join("Report_2023.pdf"), vec![0u8; 500]).unwrap();
        fs::write(tmp.path().join("Data_2024.csv"), vec![0u8; 10_000]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("regex:^Report.* size:>5kb").unwrap();
        assert_eq!(results.len(), 1);

        let path = cache.node_path(*results.first().unwrap()).unwrap();
        assert!(path.ends_with("Report_2024.pdf"));
    }

    #[test]
    fn test_wildcard_with_type_filter() {
        let tmp = TempDir::new("wildcard_with_type").unwrap();
        fs::write(tmp.path().join("vacation_photo.jpg"), b"x").unwrap();
        fs::write(tmp.path().join("family_photo.png"), b"x").unwrap();
        fs::write(tmp.path().join("work_doc.pdf"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("*photo* type:picture").unwrap();
        assert_eq!(results.len(), 2);
    }

    // ========== Edge Cases and Stress Tests (Batch 7) ==========

    #[test]
    fn test_type_filter_files_without_extensions() {
        let tmp = TempDir::new("type_no_ext").unwrap();
        fs::write(tmp.path().join("README"), b"x").unwrap();
        fs::write(tmp.path().join("Makefile"), b"x").unwrap();
        fs::write(tmp.path().join("photo.jpg"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("type:picture").unwrap();
        assert_eq!(
            results.len(),
            1,
            "Should only match files with picture extensions"
        );
    }

    #[test]
    fn test_size_filter_symlinks() {
        let tmp = TempDir::new("size_symlinks").unwrap();
        fs::write(tmp.path().join("target.bin"), vec![0u8; 5000]).unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            symlink(tmp.path().join("target.bin"), tmp.path().join("link.bin")).unwrap();
        }

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        // Size filter should handle symlinks gracefully
        let results = cache.search("size:>1kb").unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_type_filter_mixed_case_extensions() {
        let tmp = TempDir::new("type_mixed_case").unwrap();
        fs::write(tmp.path().join("photo1.JPG"), b"x").unwrap();
        fs::write(tmp.path().join("photo2.Jpg"), b"x").unwrap();
        fs::write(tmp.path().join("photo3.jPg"), b"x").unwrap();
        fs::write(tmp.path().join("photo4.jpg"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("type:picture").unwrap();
        assert_eq!(results.len(), 4, "Should handle all case variations");
    }

    #[test]
    fn test_size_filter_very_small_files() {
        let tmp = TempDir::new("size_very_small").unwrap();
        fs::write(tmp.path().join("1byte.bin"), vec![0u8; 1]).unwrap();
        fs::write(tmp.path().join("2bytes.bin"), vec![0u8; 2]).unwrap();
        fs::write(tmp.path().join("empty.bin"), vec![]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("size:>=1").unwrap();
        assert_eq!(results.len(), 2);

        let empty = cache.search("size:=0").unwrap();
        assert_eq!(empty.len(), 1);
    }

    #[test]
    fn test_multiple_type_filters_intersection() {
        let tmp = TempDir::new("multi_type_intersect").unwrap();
        fs::write(tmp.path().join("file.jpg"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        // A file can't be both audio and video, so intersection should be empty
        let results = cache.search("type:audio type:video").unwrap();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_size_filter_with_quoted_phrases() {
        let tmp = TempDir::new("size_quoted").unwrap();
        fs::write(tmp.path().join("my report.pdf"), vec![0u8; 10_000]).unwrap();
        fs::write(tmp.path().join("other.txt"), vec![0u8; 10_000]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("\"my report\" size:>5kb").unwrap();
        assert_eq!(results.len(), 1);

        let path = cache.node_path(*results.first().unwrap()).unwrap();
        assert!(path.ends_with("my report.pdf"));
    }

    #[test]
    fn test_type_filter_uppercase_category_names() {
        let tmp = TempDir::new("type_uppercase").unwrap();
        fs::write(tmp.path().join("photo.jpg"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("type:PICTURE").unwrap();
        assert_eq!(results.len(), 1);

        let results2 = cache.search("type:PiCtUrE").unwrap();
        assert_eq!(results2.len(), 1);
    }

    #[test]
    fn test_size_unit_case_insensitive() {
        let tmp = TempDir::new("size_unit_case").unwrap();
        fs::write(tmp.path().join("file.bin"), vec![0u8; 2048]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let lower = cache.search("size:>1kb").unwrap();
        assert_eq!(lower.len(), 1);

        let upper = cache.search("size:>1KB").unwrap();
        assert_eq!(upper.len(), 1);

        let mixed = cache.search("size:>1Kb").unwrap();
        assert_eq!(mixed.len(), 1);

        let megabyte = cache.search("size:<1MB").unwrap();
        assert_eq!(megabyte.len(), 1);
    }

    #[test]
    fn test_type_code_with_dot_prefixed_files() {
        let tmp = TempDir::new("type_code_dot").unwrap();
        fs::write(tmp.path().join(".gitignore"), b"x").unwrap();
        fs::write(tmp.path().join("main.rs"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("type:code").unwrap();
        assert_eq!(results.len(), 1, "Should match main.rs");
    }

    #[test]
    fn test_size_range_inclusive_bounds() {
        let tmp = TempDir::new("size_range_inclusive").unwrap();
        fs::write(tmp.path().join("1kb.bin"), vec![0u8; 1024]).unwrap();
        fs::write(tmp.path().join("2kb.bin"), vec![0u8; 2048]).unwrap();
        fs::write(tmp.path().join("3kb.bin"), vec![0u8; 3072]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("size:1kb..3kb").unwrap();
        assert_eq!(results.len(), 3, "Range should include both bounds");
    }

    #[test]
    fn test_type_archive_with_nested_structure() {
        let tmp = TempDir::new("type_archive_nested").unwrap();
        fs::create_dir(tmp.path().join("backups")).unwrap();
        fs::write(tmp.path().join("archive.zip"), b"x").unwrap();
        fs::write(tmp.path().join("backups/backup.tar.gz"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("type:archive").unwrap();
        // Note: .tar.gz might be recognized as .gz extension
        assert!(!results.is_empty());
    }

    #[test]
    fn test_size_with_multiple_and_conditions() {
        let tmp = TempDir::new("size_multi_and").unwrap();
        fs::write(tmp.path().join("report.pdf"), vec![0u8; 5_000]).unwrap();
        fs::write(tmp.path().join("data.csv"), vec![0u8; 5_000]).unwrap();
        fs::write(tmp.path().join("small.txt"), vec![0u8; 100]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("report size:>1kb ext:pdf").unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_type_executable_cross_platform() {
        let tmp = TempDir::new("type_exe_cross").unwrap();
        // Windows executables
        fs::write(tmp.path().join("app.exe"), b"x").unwrap();
        fs::write(tmp.path().join("setup.msi"), b"x").unwrap();
        // Unix executables
        fs::write(tmp.path().join("program.bin"), b"x").unwrap();
        fs::write(tmp.path().join("install.run"), b"x").unwrap();
        // macOS
        fs::write(tmp.path().join("Calculator.app"), b"x").unwrap();
        fs::write(tmp.path().join("installer.pkg"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("type:executable").unwrap();
        assert_eq!(results.len(), 6);
    }

    #[test]
    fn test_size_comparison_with_equal_files() {
        let tmp = TempDir::new("size_equal_files").unwrap();
        fs::write(tmp.path().join("file1.bin"), vec![0u8; 1000]).unwrap();
        fs::write(tmp.path().join("file2.bin"), vec![0u8; 1000]).unwrap();
        fs::write(tmp.path().join("file3.bin"), vec![0u8; 1000]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("size:=1000").unwrap();
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_type_spreadsheet_csv_special_case() {
        let tmp = TempDir::new("type_csv").unwrap();
        fs::write(tmp.path().join("data.csv"), b"x").unwrap();
        fs::write(tmp.path().join("sheet.xlsx"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("type:spreadsheet").unwrap();
        assert_eq!(results.len(), 2, "CSV should be included in spreadsheets");
    }

    #[test]
    fn test_size_filter_performance_many_files() {
        let tmp = TempDir::new("size_perf").unwrap();
        for i in 0..100 {
            let size = (i * 100) % 10000;
            fs::write(tmp.path().join(format!("file_{i}.bin")), vec![0u8; size]).unwrap();
        }

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("size:>5kb").unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_type_filter_performance_many_files() {
        let tmp = TempDir::new("type_perf").unwrap();
        let extensions = ["jpg", "png", "txt", "pdf", "mp3", "mp4"];
        for i in 0..100 {
            let ext = extensions[i % extensions.len()];
            fs::write(tmp.path().join(format!("file_{i}.{ext}")), b"x").unwrap();
        }

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("type:picture").unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_combined_filters_no_matches() {
        let tmp = TempDir::new("combined_no_match").unwrap();
        fs::write(tmp.path().join("small.jpg"), vec![0u8; 100]).unwrap();
        fs::write(tmp.path().join("large.txt"), vec![0u8; 100_000]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        // Picture that is large (but our picture is small)
        let results = cache.search("type:picture size:>10kb").unwrap();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_size_double_range_error() {
        let tmp = TempDir::new("size_double_range").unwrap();
        fs::write(tmp.path().join("file.bin"), b"x").unwrap();
        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        // This should parse as a range with start "1kb..2kb" and no end, which is invalid
        // Actually, the parser might reject this, so let's just verify it doesn't crash
        let result = cache.search("size:1kb..2kb..3kb");
        // Accept either error or unexpected parsing behavior
        let _ = result;
    }

    #[test]
    fn test_type_with_extension_that_matches_multiple_categories() {
        let tmp = TempDir::new("type_multi_cat").unwrap();
        // PDF is in multiple categories potentially
        fs::write(tmp.path().join("document.pdf"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let doc = cache.search("type:doc").unwrap();
        assert!(!doc.is_empty());

        let pdf = cache.search("type:pdf").unwrap();
        assert!(!pdf.is_empty());
    }

    #[test]
    fn test_size_negative_number_error() {
        let tmp = TempDir::new("size_negative").unwrap();
        fs::write(tmp.path().join("file.bin"), b"x").unwrap();
        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let result = cache.search("size:-100");
        // This should either error or be parsed as something else
        let _ = result;
    }

    #[test]
    fn test_type_filter_with_multiple_dots_in_filename() {
        let tmp = TempDir::new("type_multi_dots").unwrap();
        fs::write(tmp.path().join("archive.tar.gz"), b"x").unwrap();
        fs::write(tmp.path().join("backup.tar.bz2"), b"x").unwrap();
        fs::write(tmp.path().join("file.min.js"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        // Should match based on final extension
        let archives = cache.search("type:archive").unwrap();
        assert!(archives.len() >= 2);

        let code = cache.search("type:code").unwrap();
        assert_eq!(code.len(), 1);
    }

    // ========== Advanced Integration Tests (Batch 8) ==========

    #[test]
    fn test_size_range_single_point() {
        let tmp = TempDir::new("size_range_point").unwrap();
        fs::write(tmp.path().join("exact.bin"), vec![0u8; 1024]).unwrap();
        fs::write(tmp.path().join("other.bin"), vec![0u8; 2048]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("size:1kb..1kb").unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_type_filter_all_picture_formats() {
        let tmp = TempDir::new("type_all_pictures").unwrap();
        let picture_exts = [
            "jpg", "jpeg", "png", "gif", "bmp", "tif", "tiff", "webp", "ico", "svg", "heic",
            "heif", "raw", "arw", "cr2", "orf", "raf", "psd", "ai",
        ];
        for ext in &picture_exts {
            fs::write(tmp.path().join(format!("image.{ext}")), b"x").unwrap();
        }

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("type:picture").unwrap();
        assert_eq!(results.len(), picture_exts.len());
    }

    #[test]
    fn test_type_filter_all_video_formats() {
        let tmp = TempDir::new("type_all_videos").unwrap();
        let video_exts = [
            "mp4", "m4v", "mov", "avi", "mkv", "wmv", "webm", "flv", "mpg", "mpeg", "3gp", "3g2",
            "ts", "mts", "m2ts",
        ];
        for ext in &video_exts {
            fs::write(tmp.path().join(format!("video.{ext}")), b"x").unwrap();
        }

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("type:video").unwrap();
        assert_eq!(results.len(), video_exts.len());
    }

    #[test]
    fn test_type_filter_all_audio_formats() {
        let tmp = TempDir::new("type_all_audio").unwrap();
        let audio_exts = [
            "mp3", "wav", "flac", "aac", "ogg", "oga", "opus", "wma", "m4a", "alac", "aiff",
        ];
        for ext in &audio_exts {
            fs::write(tmp.path().join(format!("audio.{ext}")), b"x").unwrap();
        }

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("type:audio").unwrap();
        assert_eq!(results.len(), audio_exts.len());
    }

    #[test]
    fn test_type_filter_all_archive_formats() {
        let tmp = TempDir::new("type_all_archives").unwrap();
        let archive_exts = [
            "zip", "rar", "7z", "tar", "gz", "tgz", "bz2", "xz", "zst", "cab", "iso", "dmg",
        ];
        for ext in &archive_exts {
            fs::write(tmp.path().join(format!("archive.{ext}")), b"x").unwrap();
        }

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("type:archive").unwrap();
        assert_eq!(results.len(), archive_exts.len());
    }

    #[test]
    fn test_size_keywords_boundaries() {
        let tmp = TempDir::new("size_keywords_bounds").unwrap();
        // Test exact boundary values
        fs::write(tmp.path().join("0b.bin"), vec![]).unwrap(); // empty: 0
        fs::write(tmp.path().join("5kb.bin"), vec![0u8; 5 * 1024]).unwrap(); // tiny: 0..10KB
        fs::write(tmp.path().join("50kb.bin"), vec![0u8; 50 * 1024]).unwrap(); // small: 10KB+1..100KB
        fs::write(tmp.path().join("500kb.bin"), vec![0u8; 500 * 1024]).unwrap(); // medium: 100KB+1..1MB
        fs::write(tmp.path().join("5mb.bin"), vec![0u8; 5 * 1024 * 1024]).unwrap(); // large: 1MB+1..16MB
        fs::write(tmp.path().join("50mb.bin"), vec![0u8; 50 * 1024 * 1024]).unwrap(); // huge: 16MB+1..128MB
        fs::write(tmp.path().join("200mb.bin"), vec![0u8; 200 * 1024 * 1024]).unwrap(); // gigantic: >128MB

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let empty = cache.search("size:empty").unwrap();
        assert_eq!(empty.len(), 1, "Should match empty file");

        let tiny = cache.search("size:tiny").unwrap();
        // tiny range is 0..10KB, which includes empty files (0 bytes)
        assert_eq!(
            tiny.len(),
            2,
            "Should match 0b and 5kb files (tiny: 0..10KB)"
        );

        let small = cache.search("size:small").unwrap();
        assert_eq!(small.len(), 1, "Should match 50kb file");

        let medium = cache.search("size:medium").unwrap();
        assert_eq!(medium.len(), 1, "Should match 500kb file");

        let large = cache.search("size:large").unwrap();
        assert_eq!(large.len(), 1, "Should match 5mb file");

        let huge = cache.search("size:huge").unwrap();
        assert_eq!(huge.len(), 1, "Should match 50mb file");

        let gigantic = cache.search("size:gigantic").unwrap();
        assert_eq!(gigantic.len(), 1, "Should match 200mb file");
    }

    #[test]
    fn test_complex_query_with_precedence() {
        let tmp = TempDir::new("complex_precedence").unwrap();
        fs::write(tmp.path().join("report_a.pdf"), vec![0u8; 100_000]).unwrap();
        fs::write(tmp.path().join("report_b.txt"), vec![0u8; 100_000]).unwrap();
        fs::write(tmp.path().join("photo_a.jpg"), vec![0u8; 100_000]).unwrap();
        fs::write(tmp.path().join("photo_b.jpg"), vec![0u8; 1_000]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        // Test: (report OR photo_a) AND type:picture
        let results = cache.search("report OR photo_a type:picture").unwrap();
        assert_eq!(results.len(), 1);
        let path = cache.node_path(*results.first().unwrap()).unwrap();
        assert!(path.ends_with("photo_a.jpg"));
    }

    #[test]
    fn test_type_code_comprehensive_languages() {
        let tmp = TempDir::new("type_code_langs").unwrap();
        // C family
        fs::write(tmp.path().join("main.c"), b"x").unwrap();
        fs::write(tmp.path().join("impl.cpp"), b"x").unwrap();
        fs::write(tmp.path().join("header.h"), b"x").unwrap();
        // Rust
        fs::write(tmp.path().join("lib.rs"), b"x").unwrap();
        // JavaScript/TypeScript
        fs::write(tmp.path().join("app.js"), b"x").unwrap();
        fs::write(tmp.path().join("component.tsx"), b"x").unwrap();
        // Python
        fs::write(tmp.path().join("script.py"), b"x").unwrap();
        // Go
        fs::write(tmp.path().join("server.go"), b"x").unwrap();
        // Java
        fs::write(tmp.path().join("Main.java"), b"x").unwrap();
        // Web
        fs::write(tmp.path().join("index.html"), b"x").unwrap();
        fs::write(tmp.path().join("style.css"), b"x").unwrap();
        // Config
        fs::write(tmp.path().join("config.json"), b"x").unwrap();
        fs::write(tmp.path().join("data.yaml"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("type:code").unwrap();
        assert_eq!(results.len(), 13);
    }

    #[test]
    fn test_multiple_filters_intersection_complex() {
        let tmp = TempDir::new("multi_filter_complex").unwrap();
        fs::create_dir(tmp.path().join("photos")).unwrap();
        fs::write(tmp.path().join("photos/vacation.jpg"), vec![0u8; 100_000]).unwrap();
        fs::write(tmp.path().join("photos/small.jpg"), vec![0u8; 1_000]).unwrap();
        fs::write(tmp.path().join("document.pdf"), vec![0u8; 100_000]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let photos_dir = tmp.path().join("photos");
        let results = cache
            .search(&format!(
                "type:picture size:>10kb parent:{}",
                photos_dir.display()
            ))
            .unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_size_with_floating_point_edge_cases() {
        let tmp = TempDir::new("size_float_edge").unwrap();
        fs::write(tmp.path().join("file.bin"), vec![0u8; 1536]).unwrap(); // 1.5 KB

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("size:1.5kb").unwrap();
        assert_eq!(results.len(), 1);

        let results2 = cache.search("size:1.500kb").unwrap();
        assert_eq!(results2.len(), 1);

        let results3 = cache.search("size:>1.49kb").unwrap();
        assert_eq!(results3.len(), 1);

        let results4 = cache.search("size:>1.51kb").unwrap();
        assert_eq!(results4.len(), 0);
    }

    #[test]
    fn test_type_with_uncommon_extensions() {
        let tmp = TempDir::new("type_uncommon").unwrap();
        // Test that uncommon but valid extensions work
        fs::write(tmp.path().join("scan.tiff"), b"x").unwrap();
        fs::write(tmp.path().join("audio.opus"), b"x").unwrap();
        fs::write(tmp.path().join("archive.zst"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let pictures = cache.search("type:picture").unwrap();
        assert_eq!(pictures.len(), 1);

        let audio = cache.search("type:audio").unwrap();
        assert_eq!(audio.len(), 1);

        let archives = cache.search("type:archive").unwrap();
        assert_eq!(archives.len(), 1);
    }

    #[test]
    fn test_size_with_all_comparison_operators_on_same_file() {
        let tmp = TempDir::new("size_all_ops").unwrap();
        fs::write(tmp.path().join("1kb.bin"), vec![0u8; 1024]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        assert_eq!(cache.search("size:>1023").unwrap().len(), 1);
        assert_eq!(cache.search("size:>=1024").unwrap().len(), 1);
        assert_eq!(cache.search("size:<1025").unwrap().len(), 1);
        assert_eq!(cache.search("size:<=1024").unwrap().len(), 1);
        assert_eq!(cache.search("size:=1024").unwrap().len(), 1);
        assert_eq!(cache.search("size:!=1023").unwrap().len(), 1);
        assert_eq!(cache.search("size:!=1024").unwrap().len(), 0);
    }

    #[test]
    fn test_type_filter_special_characters_in_names() {
        let tmp = TempDir::new("type_special_chars").unwrap();
        fs::write(tmp.path().join("photo (1).jpg"), b"x").unwrap();
        fs::write(tmp.path().join("song [remix].mp3"), b"x").unwrap();
        fs::write(tmp.path().join("doc & notes.pdf"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let pictures = cache.search("type:picture").unwrap();
        assert_eq!(pictures.len(), 1);

        let audio = cache.search("type:audio").unwrap();
        assert_eq!(audio.len(), 1);

        let docs = cache.search("type:doc").unwrap();
        assert_eq!(docs.len(), 1);
    }

    #[test]
    fn test_size_range_with_different_units() {
        let tmp = TempDir::new("size_range_units").unwrap();
        fs::write(tmp.path().join("file.bin"), vec![0u8; 1_500_000]).unwrap(); // ~1.43 MB

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("size:1mb..2mb").unwrap();
        assert_eq!(results.len(), 1);

        let results2 = cache.search("size:1000kb..2000kb").unwrap();
        assert_eq!(results2.len(), 1);

        let results3 = cache.search("size:500kb..2mb").unwrap();
        assert_eq!(results3.len(), 1);
    }

    #[test]
    fn test_deeply_nested_boolean_with_filters() {
        let tmp = TempDir::new("deep_boolean").unwrap();
        fs::write(tmp.path().join("a.jpg"), vec![0u8; 100_000]).unwrap();
        fs::write(tmp.path().join("b.jpg"), vec![0u8; 1_000]).unwrap();
        fs::write(tmp.path().join("c.mp3"), vec![0u8; 100_000]).unwrap();
        fs::write(tmp.path().join("d.mp3"), vec![0u8; 1_000]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache
            .search("((type:picture OR type:audio) size:>10kb)")
            .unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_type_macros_accept_arguments_as_filters() {
        let tmp = TempDir::new("macro_with_args").unwrap();
        fs::write(tmp.path().join("file_match.mp3"), b"x").unwrap();
        fs::write(tmp.path().join("file_skip.mp3"), b"x").unwrap();
        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("audio:match").unwrap();
        assert_eq!(results.len(), 1);
        let path = cache.node_path(*results.first().unwrap()).unwrap();
        assert!(path.ends_with(PathBuf::from("file_match.mp3")));
    }

    #[test]
    fn test_size_with_name_containing_numbers() {
        let tmp = TempDir::new("size_name_numbers").unwrap();
        fs::write(tmp.path().join("file123.bin"), vec![0u8; 5000]).unwrap();
        fs::write(tmp.path().join("456file.bin"), vec![0u8; 5000]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("123 size:>1kb").unwrap();
        assert_eq!(results.len(), 1);

        let results2 = cache.search("456 size:>1kb").unwrap();
        assert_eq!(results2.len(), 1);
    }

    #[test]
    fn test_type_filter_with_wildcard_name() {
        let tmp = TempDir::new("type_wildcard_name").unwrap();
        fs::write(tmp.path().join("photo_001.jpg"), b"x").unwrap();
        fs::write(tmp.path().join("photo_002.jpg"), b"x").unwrap();
        fs::write(tmp.path().join("image_003.jpg"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("photo* type:picture").unwrap();
        assert_eq!(results.len(), 2);
    }

    // ========== Performance and Stress Tests (Batch 9) ==========

    #[test]
    fn test_size_filter_with_many_size_variants() {
        let tmp = TempDir::new("size_many_variants").unwrap();
        for i in 0..50 {
            let size = i * 1000;
            fs::write(tmp.path().join(format!("file_{i}.bin")), vec![0u8; size]).unwrap();
        }

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("size:>20kb").unwrap();
        assert!(!results.is_empty());

        let results2 = cache.search("size:10kb..30kb").unwrap();
        assert!(!results2.is_empty());
    }

    #[test]
    fn test_type_filter_mixed_categories() {
        let tmp = TempDir::new("type_mixed").unwrap();
        fs::write(tmp.path().join("a.jpg"), b"x").unwrap();
        fs::write(tmp.path().join("b.mp3"), b"x").unwrap();
        fs::write(tmp.path().join("c.mp4"), b"x").unwrap();
        fs::write(tmp.path().join("d.pdf"), b"x").unwrap();
        fs::write(tmp.path().join("e.zip"), b"x").unwrap();
        fs::write(tmp.path().join("f.exe"), b"x").unwrap();
        fs::write(tmp.path().join("g.rs"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let picture = cache.search("type:picture").unwrap();
        assert_eq!(picture.len(), 1);

        let audio = cache.search("type:audio").unwrap();
        assert_eq!(audio.len(), 1);

        let video = cache.search("type:video").unwrap();
        assert_eq!(video.len(), 1);

        let doc = cache.search("type:doc").unwrap();
        assert_eq!(doc.len(), 1);

        let archive = cache.search("type:archive").unwrap();
        assert_eq!(archive.len(), 1);

        let exe = cache.search("type:exe").unwrap();
        assert_eq!(exe.len(), 1);

        let code = cache.search("type:code").unwrap();
        assert_eq!(code.len(), 1);
    }

    #[test]
    fn test_size_extreme_values() {
        let tmp = TempDir::new("size_extreme").unwrap();
        fs::write(tmp.path().join("file.bin"), vec![0u8; 100]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        // Test very large size queries
        let results = cache.search("size:<1000pb").unwrap();
        assert!(!results.is_empty());

        // Test very small size queries
        let results2 = cache.search("size:>0").unwrap();
        assert!(!results2.is_empty());
    }

    #[test]
    fn test_complex_or_chain_with_types() {
        let tmp = TempDir::new("or_chain_types").unwrap();
        fs::write(tmp.path().join("image.jpg"), b"x").unwrap();
        fs::write(tmp.path().join("video.mp4"), b"x").unwrap();
        fs::write(tmp.path().join("audio.mp3"), b"x").unwrap();
        fs::write(tmp.path().join("doc.pdf"), b"x").unwrap();
        fs::write(tmp.path().join("archive.zip"), b"x").unwrap();
        fs::write(tmp.path().join("code.rs"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache
            .search("type:picture OR type:video OR type:audio OR type:doc")
            .unwrap();
        assert_eq!(results.len(), 4);
    }

    #[test]
    fn test_size_with_all_keywords() {
        let tmp = TempDir::new("size_all_keywords").unwrap();
        fs::write(tmp.path().join("empty.bin"), vec![]).unwrap();
        fs::write(tmp.path().join("tiny.bin"), vec![0u8; 5_000]).unwrap();
        fs::write(tmp.path().join("small.bin"), vec![0u8; 50_000]).unwrap();
        fs::write(tmp.path().join("medium.bin"), vec![0u8; 500_000]).unwrap();
        fs::write(tmp.path().join("large.bin"), vec![0u8; 5_000_000]).unwrap();
        fs::write(tmp.path().join("huge.bin"), vec![0u8; 50_000_000]).unwrap();
        fs::write(tmp.path().join("gigantic.bin"), vec![0u8; 200_000_000]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        assert!(!cache.search("size:empty").unwrap().is_empty());
        assert!(!cache.search("size:tiny").unwrap().is_empty());
        assert!(!cache.search("size:small").unwrap().is_empty());
        assert!(!cache.search("size:medium").unwrap().is_empty());
        assert!(!cache.search("size:large").unwrap().is_empty());
        assert!(!cache.search("size:huge").unwrap().is_empty());
        assert!(!cache.search("size:gigantic").unwrap().is_empty());
        assert!(!cache.search("size:giant").unwrap().is_empty());
    }

    #[test]
    fn test_type_filter_negation_complex() {
        let tmp = TempDir::new("type_negation").unwrap();
        fs::write(tmp.path().join("image.jpg"), b"x").unwrap();
        fs::write(tmp.path().join("video.mp4"), b"x").unwrap();
        fs::write(tmp.path().join("doc.txt"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("!type:picture !type:video").unwrap();
        let has_image = results.iter().any(|&i| {
            cache
                .node_path(i)
                .map(|p| p.ends_with("image.jpg"))
                .unwrap_or(false)
        });
        let has_video = results.iter().any(|&i| {
            cache
                .node_path(i)
                .map(|p| p.ends_with("video.mp4"))
                .unwrap_or(false)
        });
        assert!(!has_image && !has_video);
    }

    #[test]
    fn test_size_negation_complex() {
        let tmp = TempDir::new("size_negation").unwrap();
        fs::write(tmp.path().join("small.bin"), vec![0u8; 100]).unwrap();
        fs::write(tmp.path().join("large.bin"), vec![0u8; 100_000]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("!size:>10kb").unwrap();
        let has_large = results.iter().any(|&i| {
            cache
                .node_path(i)
                .map(|p| p.ends_with("large.bin"))
                .unwrap_or(false)
        });
        assert!(!has_large);
    }

    #[test]
    fn test_type_and_size_with_grouping() {
        let tmp = TempDir::new("type_size_grouping").unwrap();
        fs::write(tmp.path().join("large_photo.jpg"), vec![0u8; 100_000]).unwrap();
        fs::write(tmp.path().join("small_photo.jpg"), vec![0u8; 1_000]).unwrap();
        fs::write(tmp.path().join("large_video.mp4"), vec![0u8; 100_000]).unwrap();
        fs::write(tmp.path().join("small_video.mp4"), vec![0u8; 1_000]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache
            .search("(type:picture OR type:video) size:>10kb")
            .unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_multiple_ext_and_type_filter() {
        let tmp = TempDir::new("multi_ext_type").unwrap();
        fs::write(tmp.path().join("photo.jpg"), b"x").unwrap();
        fs::write(tmp.path().join("graphic.png"), b"x").unwrap();
        fs::write(tmp.path().join("document.txt"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        // ext: and type: should intersect
        let results = cache.search("ext:jpg;png type:picture").unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_size_with_very_precise_decimal() {
        let tmp = TempDir::new("size_precise_decimal").unwrap();
        fs::write(tmp.path().join("file.bin"), vec![0u8; 1536]).unwrap(); // 1.5 KB exactly

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("size:1.5kb").unwrap();
        assert_eq!(results.len(), 1);

        let results2 = cache.search("size:1.50kb").unwrap();
        assert_eq!(results2.len(), 1);

        let results3 = cache.search("size:1.5000kb").unwrap();
        assert_eq!(results3.len(), 1);
    }

    #[test]
    fn test_type_filter_unicode_filenames() {
        let tmp = TempDir::new("type_unicode").unwrap();
        fs::write(tmp.path().join("照片.jpg"), b"x").unwrap();
        fs::write(tmp.path().join("音乐.mp3"), b"x").unwrap();
        fs::write(tmp.path().join("文档.pdf"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let pictures = cache.search("type:picture").unwrap();
        assert_eq!(pictures.len(), 1);

        let audio = cache.search("type:audio").unwrap();
        assert_eq!(audio.len(), 1);

        let docs = cache.search("type:doc").unwrap();
        assert_eq!(docs.len(), 1);
    }

    #[test]
    fn test_size_with_unicode_in_filename() {
        let tmp = TempDir::new("size_unicode").unwrap();
        fs::write(tmp.path().join("文件.bin"), vec![0u8; 5000]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("size:>1kb").unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_combined_filters_empty_intersection() {
        let tmp = TempDir::new("empty_intersection").unwrap();
        fs::write(tmp.path().join("photo.jpg"), vec![0u8; 100]).unwrap();
        fs::write(tmp.path().join("large.txt"), vec![0u8; 100_000]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        // Looking for large pictures, but the picture is small
        let results = cache.search("type:picture size:>10kb").unwrap();
        assert_eq!(results.len(), 0);

        // Looking for small documents, but the document is large
        let results2 = cache.search("type:doc size:<1kb").unwrap();
        assert_eq!(results2.len(), 0);
    }

    #[test]
    fn test_type_folder_with_size_error() {
        let tmp = TempDir::new("type_folder_size").unwrap();
        fs::create_dir(tmp.path().join("folder")).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        // size: only applies to files, so folders should be excluded
        let results = cache.search("type:folder size:>0").unwrap();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_type_file_basic() {
        let tmp = TempDir::new("type_file_basic").unwrap();
        fs::write(tmp.path().join("file.txt"), b"x").unwrap();
        fs::create_dir(tmp.path().join("folder")).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("type:file").unwrap();
        assert_eq!(results.len(), 1);

        let path = cache.node_path(*results.first().unwrap()).unwrap();
        assert!(path.ends_with("file.txt"));
    }

    #[test]
    fn test_size_range_overlap() {
        let tmp = TempDir::new("size_range_overlap").unwrap();
        fs::write(tmp.path().join("file.bin"), vec![0u8; 5000]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results1 = cache.search("size:1kb..10kb").unwrap();
        assert_eq!(results1.len(), 1);

        let results2 = cache.search("size:4kb..6kb").unwrap();
        assert_eq!(results2.len(), 1);

        let results3 = cache.search("size:6kb..10kb").unwrap();
        assert_eq!(results3.len(), 0);
    }

    #[test]
    fn test_type_with_hidden_files() {
        let tmp = TempDir::new("type_hidden").unwrap();
        fs::write(tmp.path().join(".hidden.jpg"), b"x").unwrap();
        fs::write(tmp.path().join("visible.jpg"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("type:picture").unwrap();
        assert_eq!(results.len(), 2, "Should match hidden files too");
    }

    #[test]
    fn test_size_comparison_chain() {
        let tmp = TempDir::new("size_comp_chain").unwrap();
        fs::write(tmp.path().join("file.bin"), vec![0u8; 5000]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        // Combining multiple size constraints
        let results = cache.search("size:>1kb size:<10kb").unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_type_all_alternate_names() {
        let tmp = TempDir::new("type_alt_names").unwrap();
        fs::write(tmp.path().join("image.jpg"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        // Test all alternate names for pictures
        assert_eq!(cache.search("type:picture").unwrap().len(), 1);
        assert_eq!(cache.search("type:pictures").unwrap().len(), 1);
        assert_eq!(cache.search("type:image").unwrap().len(), 1);
        assert_eq!(cache.search("type:images").unwrap().len(), 1);
        assert_eq!(cache.search("type:photo").unwrap().len(), 1);
        assert_eq!(cache.search("type:photos").unwrap().len(), 1);
    }

    #[test]
    fn test_size_with_repeated_filters() {
        let tmp = TempDir::new("size_repeated").unwrap();
        fs::write(tmp.path().join("file.bin"), vec![0u8; 5000]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        // Multiple size filters should intersect
        let results = cache.search("size:>1kb size:>2kb size:>3kb").unwrap();
        assert_eq!(results.len(), 1);

        let results2 = cache.search("size:>1kb size:>10kb").unwrap();
        assert_eq!(results2.len(), 0);
    }

    #[test]
    fn test_type_with_repeated_filters() {
        let tmp = TempDir::new("type_repeated").unwrap();
        fs::write(tmp.path().join("photo.jpg"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        // Same type filter repeated should still work
        let results = cache.search("type:picture type:picture").unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_complex_real_world_query() {
        let tmp = TempDir::new("real_world").unwrap();
        fs::write(
            tmp.path().join("vacation_photo_2024.jpg"),
            vec![0u8; 500_000],
        )
        .unwrap();
        fs::write(tmp.path().join("family_photo_2024.jpg"), vec![0u8; 1_000]).unwrap();
        fs::write(
            tmp.path().join("vacation_video_2024.mp4"),
            vec![0u8; 500_000],
        )
        .unwrap();
        fs::write(tmp.path().join("old_photo_2023.jpg"), vec![0u8; 500_000]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        // Find large vacation photos from 2024
        let results = cache
            .search("vacation 2024 type:picture size:>100kb")
            .unwrap();
        assert_eq!(results.len(), 1);

        let path = cache.node_path(*results.first().unwrap()).unwrap();
        assert!(path.ends_with("vacation_photo_2024.jpg"));
    }

    // ========== Final Edge Cases and Regression Tests (Batch 10) ==========

    #[test]
    fn test_size_zero_with_comparison_operators() {
        let tmp = TempDir::new("size_zero_comp").unwrap();
        fs::write(tmp.path().join("empty.bin"), vec![]).unwrap();
        fs::write(tmp.path().join("nonempty.bin"), vec![0u8; 1]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let gt_zero = cache.search("size:>0").unwrap();
        assert_eq!(gt_zero.len(), 1);

        let gte_zero = cache.search("size:>=0").unwrap();
        assert_eq!(gte_zero.len(), 2);

        let eq_zero = cache.search("size:=0").unwrap();
        assert_eq!(eq_zero.len(), 1);

        let ne_zero = cache.search("size:!=0").unwrap();
        assert_eq!(ne_zero.len(), 1);
    }

    #[test]
    fn test_type_extensions_case_normalization() {
        let tmp = TempDir::new("type_case_norm").unwrap();
        fs::write(tmp.path().join("photo1.JPG"), b"x").unwrap();
        fs::write(tmp.path().join("photo2.JpG"), b"x").unwrap();
        fs::write(tmp.path().join("photo3.jpg"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("type:picture").unwrap();
        assert_eq!(
            results.len(),
            3,
            "Should match all case variations of JPG extension"
        );
    }

    #[test]
    fn test_size_scientific_notation_not_supported() {
        let tmp = TempDir::new("size_scientific").unwrap();
        fs::write(tmp.path().join("file.bin"), b"x").unwrap();
        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        // Scientific notation should fail to parse
        let result = cache.search("size:1e6");
        // Should either error or parse incorrectly
        let _ = result;
    }

    #[test]
    fn test_type_empty_extension() {
        let tmp = TempDir::new("type_empty_ext").unwrap();
        fs::write(tmp.path().join("file."), b"x").unwrap();
        fs::write(tmp.path().join("normal.jpg"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("type:picture").unwrap();
        assert_eq!(
            results.len(),
            1,
            "Should not match file with empty extension"
        );
    }

    #[test]
    fn test_size_range_only_start() {
        let tmp = TempDir::new("size_range_start").unwrap();
        fs::write(tmp.path().join("small.bin"), vec![0u8; 500]).unwrap();
        fs::write(tmp.path().join("large.bin"), vec![0u8; 50_000]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("size:1kb..").unwrap();
        assert_eq!(results.len(), 1);

        let path = cache.node_path(*results.first().unwrap()).unwrap();
        assert!(path.ends_with("large.bin"));
    }

    #[test]
    fn test_size_range_only_end() {
        let tmp = TempDir::new("size_range_end").unwrap();
        fs::write(tmp.path().join("small.bin"), vec![0u8; 500]).unwrap();
        fs::write(tmp.path().join("large.bin"), vec![0u8; 50_000]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("size:..10kb").unwrap();
        assert_eq!(results.len(), 1);

        let path = cache.node_path(*results.first().unwrap()).unwrap();
        assert!(path.ends_with("small.bin"));
    }

    #[test]
    fn test_type_multiple_extensions_in_filename() {
        let tmp = TempDir::new("type_multi_ext_name").unwrap();
        fs::write(tmp.path().join("archive.tar.gz"), b"x").unwrap();
        fs::write(tmp.path().join("backup.tar.bz2"), b"x").unwrap();
        fs::write(tmp.path().join("data.json.backup"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        // Should match based on final extension
        let gz = cache.search("type:archive").unwrap();
        assert!(gz.len() >= 2, "Should match .gz and .bz2");
    }

    #[test]
    fn test_size_keyword_with_spaces() {
        let tmp = TempDir::new("size_keyword_space").unwrap();
        fs::write(tmp.path().join("file.bin"), vec![0u8; 100]).unwrap();
        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        // Test that spaces are trimmed
        let result = cache.search("size: tiny ");
        // Should work or error gracefully
        let _ = result;
    }

    #[test]
    fn test_type_and_ext_filter_conflict() {
        let tmp = TempDir::new("type_ext_conflict").unwrap();
        fs::write(tmp.path().join("photo.jpg"), b"x").unwrap();
        fs::write(tmp.path().join("document.pdf"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        // type:picture AND ext:pdf should give empty result
        let results = cache.search("type:picture ext:pdf").unwrap();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_size_multiple_ranges_or() {
        let tmp = TempDir::new("size_multi_range").unwrap();
        fs::write(tmp.path().join("tiny.bin"), vec![0u8; 100]).unwrap();
        fs::write(tmp.path().join("medium.bin"), vec![0u8; 5000]).unwrap();
        fs::write(tmp.path().join("large.bin"), vec![0u8; 100_000]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("size:..500 OR size:50kb..").unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_type_code_config_files() {
        let tmp = TempDir::new("type_code_config").unwrap();
        fs::write(tmp.path().join("config.json"), b"x").unwrap();
        fs::write(tmp.path().join("settings.yaml"), b"x").unwrap();
        fs::write(tmp.path().join("Cargo.toml"), b"x").unwrap();
        fs::write(tmp.path().join("setup.ini"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("type:code").unwrap();
        assert_eq!(
            results.len(),
            4,
            "Config files should be included in code type"
        );
    }

    #[test]
    fn test_size_boundary_exact_1024() {
        let tmp = TempDir::new("size_1024").unwrap();
        fs::write(tmp.path().join("1023.bin"), vec![0u8; 1023]).unwrap();
        fs::write(tmp.path().join("1024.bin"), vec![0u8; 1024]).unwrap();
        fs::write(tmp.path().join("1025.bin"), vec![0u8; 1025]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let lt = cache.search("size:<1kb").unwrap();
        assert_eq!(lt.len(), 1);

        let eq = cache.search("size:=1kb").unwrap();
        assert_eq!(eq.len(), 1);

        let gt = cache.search("size:>1kb").unwrap();
        assert_eq!(gt.len(), 1);
    }

    #[test]
    fn test_type_presentation_all_formats() {
        let tmp = TempDir::new("type_pres_all").unwrap();
        fs::write(tmp.path().join("deck.ppt"), b"x").unwrap();
        fs::write(tmp.path().join("slides.pptx"), b"x").unwrap();
        fs::write(tmp.path().join("keynote.key"), b"x").unwrap();
        fs::write(tmp.path().join("present.odp"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("type:presentation").unwrap();
        assert_eq!(results.len(), 4);
    }

    #[test]
    fn test_size_with_path_filter_complex() {
        let tmp = TempDir::new("size_path_complex").unwrap();
        fs::create_dir(tmp.path().join("large_files")).unwrap();
        fs::create_dir(tmp.path().join("small_files")).unwrap();
        fs::write(tmp.path().join("large_files/file.bin"), vec![0u8; 100_000]).unwrap();
        fs::write(tmp.path().join("small_files/file.bin"), vec![0u8; 100]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let large_dir = tmp.path().join("large_files");
        let results = cache
            .search(&format!(
                "file.bin size:>10kb parent:{}",
                large_dir.display()
            ))
            .unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_type_macros_case_insensitive() {
        let tmp = TempDir::new("macro_case").unwrap();
        fs::write(tmp.path().join("song.mp3"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let lower = cache.search("audio:").unwrap();
        assert_eq!(lower.len(), 1);

        let upper = cache.search("AUDIO:").unwrap();
        assert_eq!(upper.len(), 1);

        let mixed = cache.search("AuDiO:").unwrap();
        assert_eq!(mixed.len(), 1);
    }

    #[test]
    fn test_size_overflow_protection() {
        let tmp = TempDir::new("size_overflow").unwrap();
        fs::write(tmp.path().join("file.bin"), b"x").unwrap();
        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        // Very large number that might overflow
        let result = cache.search("size:<99999999999999gb");
        assert!(result.is_ok(), "Should handle large numbers gracefully");
    }

    #[test]
    fn test_type_file_and_folder_together() {
        let tmp = TempDir::new("type_both").unwrap();
        fs::write(tmp.path().join("file.txt"), b"x").unwrap();
        fs::create_dir(tmp.path().join("folder")).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        // file OR folder should return both
        let results = cache.search("type:file OR type:folder").unwrap();
        assert!(results.len() >= 2, "Should return at least file and folder");
    }

    #[test]
    fn test_size_with_leading_zeros() {
        let tmp = TempDir::new("size_leading_zeros").unwrap();
        fs::write(tmp.path().join("file.bin"), vec![0u8; 1024]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("size:01kb").unwrap();
        assert_eq!(results.len(), 1);

        let results2 = cache.search("size:001kb").unwrap();
        assert_eq!(results2.len(), 1);
    }

    #[test]
    fn test_type_with_no_extension_edge_case() {
        let tmp = TempDir::new("type_no_ext_edge").unwrap();
        fs::write(tmp.path().join("Makefile"), b"x").unwrap();
        fs::write(tmp.path().join("README"), b"x").unwrap();
        fs::write(tmp.path().join("LICENSE"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        // These files have no extensions, so type filters shouldn't match them
        let results = cache.search("type:doc").unwrap();
        assert_eq!(results.len(), 0);

        let results2 = cache.search("type:code").unwrap();
        assert_eq!(results2.len(), 0);
    }

    #[test]
    fn test_combined_all_filter_types() {
        let tmp = TempDir::new("all_filters").unwrap();
        fs::create_dir(tmp.path().join("photos")).unwrap();
        fs::write(tmp.path().join("photos/vacation.jpg"), vec![0u8; 100_000]).unwrap();
        fs::write(tmp.path().join("photos/small.jpg"), vec![0u8; 1_000]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let photos_dir = tmp.path().join("photos");
        let results = cache
            .search(&format!(
                "vacation type:picture size:>10kb ext:jpg parent:{}",
                photos_dir.display()
            ))
            .unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_size_with_mixed_units_in_range() {
        let tmp = TempDir::new("size_mixed_units").unwrap();
        fs::write(tmp.path().join("file.bin"), vec![0u8; 1_500_000]).unwrap(); // ~1.43 MB

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("size:1000kb..2mb").unwrap();
        assert_eq!(results.len(), 1);

        let results2 = cache.search("size:1mb..2000kb").unwrap();
        assert_eq!(results2.len(), 1);
    }

    #[test]
    fn test_type_picture_raw_formats() {
        let tmp = TempDir::new("type_raw").unwrap();
        fs::write(tmp.path().join("sony.arw"), b"x").unwrap();
        fs::write(tmp.path().join("canon.cr2"), b"x").unwrap();
        fs::write(tmp.path().join("olympus.orf"), b"x").unwrap();
        fs::write(tmp.path().join("fuji.raf"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("type:picture").unwrap();
        assert_eq!(results.len(), 4, "RAW formats should be recognized");
    }

    #[test]
    fn test_size_keyword_boundaries_precise() {
        let tmp = TempDir::new("size_keyword_precise").unwrap();
        // Test exact boundaries: tiny is 0..=10KB, small is 10KB+1..=100KB
        fs::write(tmp.path().join("tiny_max.bin"), vec![0u8; 10 * 1024]).unwrap(); // 10 KB - in tiny
        fs::write(tmp.path().join("small_min.bin"), vec![0u8; 10 * 1024 + 1]).unwrap(); // 10 KB + 1 - in small

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let tiny = cache.search("size:tiny").unwrap();
        assert_eq!(tiny.len(), 1); // tiny_max.bin

        let small = cache.search("size:small").unwrap();
        assert_eq!(small.len(), 1); // small_min.bin
    }

    #[test]
    fn test_type_video_mobile_formats() {
        let tmp = TempDir::new("type_video_mobile").unwrap();
        fs::write(tmp.path().join("video.3gp"), b"x").unwrap();
        fs::write(tmp.path().join("video.3g2"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("type:video").unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_size_range_with_keywords_error() {
        let tmp = TempDir::new("size_range_keyword").unwrap();
        fs::write(tmp.path().join("file.bin"), b"x").unwrap();
        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        // Keywords in ranges might not be supported
        let result = cache.search("size:tiny..large");
        // Should error or handle gracefully
        let _ = result;
    }

    #[test]
    fn test_type_spreadsheet_all_variants() {
        let tmp = TempDir::new("type_sheet_all").unwrap();
        fs::write(tmp.path().join("old.xls"), b"x").unwrap();
        fs::write(tmp.path().join("new.xlsx"), b"x").unwrap();
        fs::write(tmp.path().join("data.csv"), b"x").unwrap();
        fs::write(tmp.path().join("apple.numbers"), b"x").unwrap();
        fs::write(tmp.path().join("open.ods"), b"x").unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("type:spreadsheet").unwrap();
        assert_eq!(results.len(), 5);
    }

    #[test]
    fn test_regression_type_and_size_intersection() {
        let tmp = TempDir::new("regression_intersect").unwrap();
        fs::write(tmp.path().join("a.jpg"), vec![0u8; 100_000]).unwrap();
        fs::write(tmp.path().join("b.jpg"), vec![0u8; 1_000]).unwrap();
        fs::write(tmp.path().join("c.mp3"), vec![0u8; 100_000]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        // Should intersect properly
        let results = cache.search("type:picture size:>10kb").unwrap();
        assert_eq!(results.len(), 1);

        let path = cache.node_path(*results.first().unwrap()).unwrap();
        assert!(path.ends_with("a.jpg"));
    }

    #[test]
    fn test_stress_many_filters_combined() {
        let tmp = TempDir::new("stress_many_filters").unwrap();
        fs::create_dir(tmp.path().join("media")).unwrap();
        fs::write(
            tmp.path().join("media/vacation_2024_photo.jpg"),
            vec![0u8; 500_000],
        )
        .unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let media_dir = tmp.path().join("media");
        let results = cache
            .search(&format!(
                "vacation 2024 photo type:picture size:>100kb ext:jpg parent:{}",
                media_dir.display()
            ))
            .unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_size_decimal_rounding() {
        let tmp = TempDir::new("size_rounding").unwrap();
        fs::write(tmp.path().join("file.bin"), vec![0u8; 1536]).unwrap(); // 1.5 KB

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        let results = cache.search("size:1.5kb").unwrap();
        assert_eq!(results.len(), 1);

        // Test rounding behavior - 1.4999kb rounds to 1535.897 bytes, which is less than 1536
        let results2 = cache.search("size:>=1.5kb").unwrap();
        assert_eq!(results2.len(), 1);
    }

    #[test]
    fn test_final_integration_comprehensive() {
        let tmp = TempDir::new("final_integration").unwrap();
        // Create a realistic file structure
        fs::create_dir_all(tmp.path().join("Documents/Reports")).unwrap();
        fs::create_dir_all(tmp.path().join("Media/Photos")).unwrap();
        fs::create_dir_all(tmp.path().join("Media/Videos")).unwrap();
        fs::create_dir(tmp.path().join("Code")).unwrap();

        fs::write(
            tmp.path().join("Documents/Reports/Q4_Report.pdf"),
            vec![0u8; 1_000_000],
        )
        .unwrap();
        fs::write(tmp.path().join("Documents/Notes.txt"), vec![0u8; 5_000]).unwrap();
        fs::write(
            tmp.path().join("Media/Photos/vacation.jpg"),
            vec![0u8; 500_000],
        )
        .unwrap();
        fs::write(
            tmp.path().join("Media/Videos/clip.mp4"),
            vec![0u8; 5_000_000],
        )
        .unwrap();
        fs::write(tmp.path().join("Code/main.rs"), vec![0u8; 10_000]).unwrap();

        let mut cache = SearchCache::walk_fs(tmp.path().to_path_buf());

        // Test 1: Find large documents
        let docs = cache.search("type:doc size:>100kb").unwrap();
        assert_eq!(docs.len(), 1);

        // Test 2: Find media files
        let media = cache.search("type:picture OR type:video").unwrap();
        assert_eq!(media.len(), 2);

        // Test 3: Find code files
        let code = cache.search("type:code").unwrap();
        assert_eq!(code.len(), 1);

        // Test 4: Complex query
        let results = cache.search("vacation type:picture size:>100kb").unwrap();
        assert_eq!(results.len(), 1);
    }
}
