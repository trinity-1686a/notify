extern crate notify;
extern crate tempdir;
extern crate tempfile;
extern crate time;

mod utils;

use notify::*;
use std::sync::mpsc;
use tempdir::TempDir;
use std::thread;

use utils::*;

#[cfg(target_os="linux")]
#[test]
fn new_inotify() {
    let (tx, _) = mpsc::channel();
    let w: Result<INotifyWatcher> = Watcher::new(tx);
    assert!(w.is_ok());
}

#[cfg(target_os="macos")]
#[test]
fn new_fsevent() {
    let (tx, _) = mpsc::channel();
    let w: Result<FsEventWatcher> = Watcher::new(tx);
    assert!(w.is_ok());
}

#[test]
fn new_null() {
    let (tx, _) = mpsc::channel();
    let w: Result<NullWatcher> = Watcher::new(tx);
    assert!(w.is_ok());
}

#[test]
fn new_poll() {
    let (tx, _) = mpsc::channel();
    let w: Result<PollWatcher> = Watcher::new(tx);
    assert!(w.is_ok());
}

#[test]
fn new_recommended() {
    let (tx, _) = mpsc::channel();
    let w: Result<RecommendedWatcher> = Watcher::new(tx);
    assert!(w.is_ok());
}

// if this test builds, it means RecommendedWatcher is Send.
#[test]
fn test_watcher_send() {
    let (tx, _) = mpsc::channel();

    let mut watcher: RecommendedWatcher = Watcher::new(tx).unwrap();

    thread::spawn(move || {
        watcher.watch(".", RecursiveMode::Recursive).unwrap();
    }).join().unwrap();
}

// if this test builds, it means RecommendedWatcher is Sync.
#[test]
fn test_watcher_sync() {
    use std::sync::{ Arc, RwLock };

    let (tx, _) = mpsc::channel();

    let watcher: RecommendedWatcher = Watcher::new(tx).unwrap();
    let watcher = Arc::new(RwLock::new(watcher));

    thread::spawn(move || {
        let mut watcher = watcher.write().unwrap();
        watcher.watch(".", RecursiveMode::Recursive).unwrap();
    }).join().unwrap();
}

#[test]
fn watch_recursive_create_directory() {
    let tdir = TempDir::new("temp_dir").expect("failed to create temporary directory");

    if cfg!(target_os="macos") {
        sleep(10);
    }

    let (tx, rx) = mpsc::channel();
    let mut watcher: RecommendedWatcher = Watcher::new(tx).expect("failed to create recommended watcher");
    watcher.watch(&tdir.mkpath("."), RecursiveMode::Recursive).expect("failed to watch directory");

    if cfg!(target_os="windows") {
        sleep(100);
    }

    tdir.create("dir1");
    sleep(10);
    tdir.create("dir1/file1");

    if cfg!(target_os="macos") {
        sleep(100);
    }

    watcher.unwatch(&tdir.mkpath(".")).expect("failed to unwatch directory");

    if cfg!(target_os="windows") {
        sleep(100);
    }

    tdir.create("dir1/file2");

    let actual = if cfg!(target_os="windows") {
        // Windows may sneak a write event in there
        let mut events = recv_events(&rx);
        events.retain(|&(_, op)| op != op::WRITE);
        events
    } else if cfg!(target_os="macos") {
        inflate_events(recv_events(&rx))
    } else {
        recv_events(&rx)
    };

    assert_eq!(actual, vec![
        (tdir.mkpath("dir1"), op::CREATE),
        (tdir.mkpath("dir1/file1"), op::CREATE)
    ]);
}

#[test]
#[ignore]
fn watch_recursive_move() {
    let tdir = TempDir::new("temp_dir").expect("failed to create temporary directory");

    tdir.create_all(vec![
        "dir1a",
    ]);

    if cfg!(target_os="macos") {
        sleep(10);
    }

    let (tx, rx) = mpsc::channel();
    let mut watcher: RecommendedWatcher = Watcher::new(tx).expect("failed to create recommended watcher");
    watcher.watch(tdir.mkpath("."), RecursiveMode::Recursive).expect("failed to watch directory");

    if cfg!(target_os="windows") {
        sleep(100);
    }

    tdir.create("dir1a/file1");
    tdir.rename("dir1a", "dir1b");
    sleep(10);
    tdir.create("dir1b/file2");

    let actual = if cfg!(target_os="windows") {
        // Windows may sneak a write event in there
        let mut events = recv_events(&rx);
        events.retain(|&(_, op)| op != op::WRITE);
        events
    } else if cfg!(target_os="macos") {
        inflate_events(recv_events(&rx))
    } else {
        recv_events(&rx)
    };

    if cfg!(target_os="windows") {
        assert_eq!(actual, vec![
            (tdir.mkpath("dir1a/file1"), op::CREATE),
            (tdir.mkpath("dir1a"), op::RENAME),
            (tdir.mkpath("dir1b"), op::RENAME), // should be a create
            (tdir.mkpath("dir1b/file2"), op::CREATE)
        ]);
        panic!("move_to should be translated to create");
    } else if cfg!(target_os="macos") {
        assert_eq!(actual, vec![
            (tdir.mkpath("dir1a/file1"), op::CREATE),
            (tdir.mkpath("dir1a"), op::CREATE | op::RENAME), // excessive create event
            (tdir.mkpath("dir1b"), op::RENAME), // should be a create
            (tdir.mkpath("dir1b/file2"), op::CREATE)
        ]);
    } else {
        assert_eq!(actual, vec![
            (tdir.mkpath("dir1a/file1"), op::CREATE),
            (tdir.mkpath("dir1a"), op::RENAME),
            (tdir.mkpath("dir1b"), op::CREATE),
            (tdir.mkpath("dir1b/file2"), op::CREATE)
        ]);
    }
}

#[test]
#[ignore]
fn watch_recursive_move_in() {
    let tdir = TempDir::new("temp_dir").expect("failed to create temporary directory");

    tdir.create_all(vec![
        "watch_dir",
        "dir1a/dir1",
    ]);

    if cfg!(target_os="macos") {
        sleep(10);
    }

    let (tx, rx) = mpsc::channel();
    let mut watcher: RecommendedWatcher = Watcher::new(tx).expect("failed to create recommended watcher");
    watcher.watch(&tdir.mkpath("watch_dir"), RecursiveMode::Recursive).expect("failed to watch directory");

    if cfg!(target_os="windows") {
        sleep(100);
    }

    tdir.rename("dir1a", "watch_dir/dir1b");
    sleep(10);
    tdir.create("watch_dir/dir1b/dir1/file1");

    let actual = if cfg!(target_os="windows") {
        // Windows may sneak a write event in there
        let mut events = recv_events(&rx);
        events.retain(|&(_, op)| op != op::WRITE);
        events
    } else if cfg!(target_os="macos") {
        inflate_events(recv_events(&rx))
    } else {
        recv_events(&rx)
    };

    if cfg!(target_os="macos") {
        assert_eq!(actual, vec![
            (tdir.mkpath("watch_dir/dir1b"), op::RENAME), // fsevents interprets a move_to as a rename event
            (tdir.mkpath("watch_dir/dir1b/dir1/file1"), op::CREATE)
        ]);
        panic!("move event should be a create event");
    } else {
        assert_eq!(actual, vec![
            (tdir.mkpath("watch_dir/dir1b"), op::CREATE),
            (tdir.mkpath("watch_dir/dir1b/dir1/file1"), op::CREATE)
        ]);
    }
}

#[test]
fn watch_recursive_move_out() {
    let tdir = TempDir::new("temp_dir").expect("failed to create temporary directory");

    tdir.create_all(vec![
        "watch_dir/dir1a/dir1",
    ]);

    if cfg!(target_os="macos") {
        sleep(10);
    }

    let (tx, rx) = mpsc::channel();
    let mut watcher: RecommendedWatcher = Watcher::new(tx).expect("failed to create recommended watcher");
    watcher.watch(tdir.mkpath("watch_dir"), RecursiveMode::Recursive).expect("failed to watch directory");

    if cfg!(target_os="windows") {
        sleep(100);
    }

    tdir.create("watch_dir/dir1a/dir1/file1");
    tdir.rename("watch_dir/dir1a", "dir1b");
    sleep(10);
    tdir.create("dir1b/dir1/file2");

    let actual = if cfg!(target_os="windows") {
        // Windows may sneak a write event in there
        let mut events = recv_events(&rx);
        events.retain(|&(_, op)| op != op::WRITE);
        events
    } else if cfg!(target_os="macos") {
        inflate_events(recv_events(&rx))
    } else {
        recv_events(&rx)
    };

    if cfg!(target_os="windows") {
        assert_eq!(actual, vec![
            (tdir.mkpath("watch_dir/dir1a/dir1/file1"), op::CREATE),
            (tdir.mkpath("watch_dir/dir1a"), op::REMOVE) // windows interprets a move out of the watched directory as a remove
        ]);
    } else if cfg!(target_os="macos") {
        assert_eq!(actual, vec![
            (tdir.mkpath("watch_dir/dir1a/dir1/file1"), op::CREATE),
            (tdir.mkpath("watch_dir/dir1a"), op::CREATE | op::RENAME) // excessive create event
        ]);
    } else {
        assert_eq!(actual, vec![
            (tdir.mkpath("watch_dir/dir1a/dir1/file1"), op::CREATE),
            (tdir.mkpath("watch_dir/dir1a"), op::RENAME)
        ]);
    }
}

#[test]
fn watch_nonrecursive() {
    let tdir = TempDir::new("temp_dir").expect("failed to create temporary directory");

    tdir.create_all(vec![
        "dir1",
    ]);

    if cfg!(target_os="macos") {
        sleep(10);
    }

    let (tx, rx) = mpsc::channel();
    let mut watcher: RecommendedWatcher = Watcher::new(tx).expect("failed to create recommended watcher");
    watcher.watch(tdir.mkpath("."), RecursiveMode::NonRecursive).expect("failed to watch directory");

    if cfg!(target_os="windows") {
        sleep(100);
    }

    tdir.create("dir2");
    sleep(10);
    tdir.create_all(vec![
        "file0",
        "dir1/file1",
        "dir2/file2",
    ]);

    assert_eq!(recv_events(&rx), vec![
        (tdir.mkpath("dir2"), op::CREATE),
        (tdir.mkpath("file0"), op::CREATE)
    ]);
}

#[test]
fn watch_file() {
    let tdir = TempDir::new("temp_dir").expect("failed to create temporary directory");

    tdir.create_all(vec![
        "file1",
    ]);

    if cfg!(target_os="macos") {
        sleep(10);
    }

    let (tx, rx) = mpsc::channel();
    let mut watcher: RecommendedWatcher = Watcher::new(tx).expect("failed to create recommended watcher");
    watcher.watch(tdir.mkpath("file1"), RecursiveMode::Recursive).expect("failed to watch directory");

    if cfg!(target_os="windows") {
        sleep(100);
    }

    tdir.write("file1");
    tdir.create("file2");

    if cfg!(target_os="macos") {
        assert_eq!(recv_events(&rx), vec![
            (tdir.mkpath("file1"), op::CREATE | op::WRITE)
        ]);
    } else {
        assert_eq!(recv_events(&rx), vec![
            (tdir.mkpath("file1"), op::WRITE)
        ]);
    }
}

#[test]
fn poll_watch_recursive_create_directory() {
    let tdir = TempDir::new("temp_dir").expect("failed to create temporary directory");

    let (tx, rx) = mpsc::channel();
    let mut watcher = PollWatcher::with_delay(tx, 50).expect("failed to create poll watcher");
    watcher.watch(tdir.mkpath("."), RecursiveMode::Recursive).expect("failed to watch directory");

    sleep(1100); // PollWatcher has only a resolution of 1 second

    tdir.create("dir1/file1");

    sleep(1100); // PollWatcher has only a resolution of 1 second

    watcher.unwatch(tdir.mkpath(".")).expect("failed to unwatch directory");

    tdir.create("dir1/file2");

    let mut actual = recv_events(&rx);
    actual.sort_by(|a, b| a.0.cmp(&b.0));

    assert_eq!(actual, vec![
        (tdir.mkpath("."), op::WRITE), // parent directory gets modified
        (tdir.mkpath("dir1"), op::CREATE),
        (tdir.mkpath("dir1/file1"), op::CREATE)
    ]);
}

#[test]
fn poll_watch_recursive_move() {
    let tdir = TempDir::new("temp_dir").expect("failed to create temporary directory");

    tdir.create_all(vec![
        "dir1a",
    ]);

    let (tx, rx) = mpsc::channel();
    let mut watcher = PollWatcher::with_delay(tx, 50).expect("failed to create poll watcher");
    watcher.watch(tdir.mkpath("."), RecursiveMode::Recursive).expect("failed to watch directory");

    sleep(1100); // PollWatcher has only a resolution of 1 second

    tdir.create("dir1a/file1");

    let mut actual = recv_events(&rx);
    actual.sort_by(|a, b| a.0.cmp(&b.0));

    assert_eq!(actual, vec![
        (tdir.mkpath("dir1a"), op::WRITE), // parent directory gets modified
        (tdir.mkpath("dir1a/file1"), op::CREATE),
    ]);

    sleep(1100); // PollWatcher has only a resolution of 1 second

    tdir.rename("dir1a", "dir1b");
    tdir.create("dir1b/file2");

    actual = recv_events(&rx);
    actual.sort_by(|a, b| a.0.cmp(&b.0));

    if cfg!(target_os="windows") {
        assert_eq!(actual, vec![
            (tdir.mkpath("."), op::WRITE), // parent directory gets modified
            (tdir.mkpath("dir1a"), op::REMOVE),
            (tdir.mkpath("dir1a/file1"), op::REMOVE),
            (tdir.mkpath("dir1b"), op::CREATE),
            (tdir.mkpath("dir1b"), op::WRITE), // parent directory gets modified
            (tdir.mkpath("dir1b/file1"), op::CREATE),
            (tdir.mkpath("dir1b/file2"), op::CREATE),
        ]);
    } else {
        assert_eq!(actual, vec![
            (tdir.mkpath("."), op::WRITE), // parent directory gets modified
            (tdir.mkpath("dir1a"), op::REMOVE),
            (tdir.mkpath("dir1a/file1"), op::REMOVE),
            (tdir.mkpath("dir1b"), op::CREATE),
            (tdir.mkpath("dir1b/file1"), op::CREATE),
            (tdir.mkpath("dir1b/file2"), op::CREATE),
        ]);
    }
}

#[test]
fn poll_watch_recursive_move_in() {
    let tdir = TempDir::new("temp_dir").expect("failed to create temporary directory");

    tdir.create_all(vec![
        "watch_dir",
        "dir1a/dir1",
    ]);

    let (tx, rx) = mpsc::channel();
    let mut watcher = PollWatcher::with_delay(tx, 50).expect("failed to create poll watcher");
    watcher.watch(tdir.mkpath("watch_dir"), RecursiveMode::Recursive).expect("failed to watch directory");

    sleep(1100); // PollWatcher has only a resolution of 1 second

    tdir.rename("dir1a", "watch_dir/dir1b");
    tdir.create("watch_dir/dir1b/dir1/file1");

    let mut actual = recv_events(&rx);
    actual.sort_by(|a, b| a.0.cmp(&b.0));

    if cfg!(target_os="windows") {
        assert_eq!(actual, vec![
            (tdir.mkpath("watch_dir"), op::WRITE), // parent directory gets modified
            (tdir.mkpath("watch_dir/dir1b"), op::CREATE),
            (tdir.mkpath("watch_dir/dir1b/dir1"), op::CREATE),
            (tdir.mkpath("watch_dir/dir1b/dir1"), op::WRITE), // extra write event
            (tdir.mkpath("watch_dir/dir1b/dir1/file1"), op::CREATE),
        ]);
    } else {
        assert_eq!(actual, vec![
            (tdir.mkpath("watch_dir"), op::WRITE), // parent directory gets modified
            (tdir.mkpath("watch_dir/dir1b"), op::CREATE),
            (tdir.mkpath("watch_dir/dir1b/dir1"), op::CREATE),
            (tdir.mkpath("watch_dir/dir1b/dir1/file1"), op::CREATE),
        ]);
    }
}

#[test]
fn poll_watch_recursive_move_out() {
    let tdir = TempDir::new("temp_dir").expect("failed to create temporary directory");

    tdir.create_all(vec![
        "watch_dir/dir1a/dir1",
    ]);

    let (tx, rx) = mpsc::channel();
    let mut watcher = PollWatcher::with_delay(tx, 50).expect("failed to create poll watcher");
    watcher.watch(tdir.mkpath("watch_dir"), RecursiveMode::Recursive).expect("failed to watch directory");

    sleep(1100); // PollWatcher has only a resolution of 1 second

    tdir.create("watch_dir/dir1a/dir1/file1");

    let mut actual = recv_events(&rx);
    actual.sort_by(|a, b| a.0.cmp(&b.0));

    assert_eq!(actual, vec![
        (tdir.mkpath("watch_dir/dir1a/dir1"), op::WRITE), // parent directory gets modified
        (tdir.mkpath("watch_dir/dir1a/dir1/file1"), op::CREATE),
    ]);

    sleep(1100); // PollWatcher has only a resolution of 1 second

    tdir.rename("watch_dir/dir1a", "dir1b");
    tdir.create("dir1b/dir1/file2");

    actual = recv_events(&rx);
    actual.sort_by(|a, b| a.0.cmp(&b.0));

    assert_eq!(actual, vec![
        (tdir.mkpath("watch_dir"), op::WRITE), // parent directory gets modified
        (tdir.mkpath("watch_dir/dir1a"), op::REMOVE),
        (tdir.mkpath("watch_dir/dir1a/dir1"), op::REMOVE),
        (tdir.mkpath("watch_dir/dir1a/dir1/file1"), op::REMOVE),
    ]);
}

#[test]
fn poll_watch_nonrecursive() {
    let tdir = TempDir::new("temp_dir").expect("failed to create temporary directory");

    tdir.create_all(vec![
        "dir1",
    ]);

    let (tx, rx) = mpsc::channel();
    let mut watcher = PollWatcher::with_delay(tx, 50).expect("failed to create poll watcher");
    watcher.watch(tdir.mkpath("."), RecursiveMode::NonRecursive).expect("failed to watch directory");

    sleep(1100); // PollWatcher has only a resolution of 1 second

    tdir.create_all(vec![
        "file1",
        "dir1/file1",
        "dir2/file1",
    ]);

    let mut actual = recv_events(&rx);
    actual.sort_by(|a, b| a.0.cmp(&b.0));

    assert_eq!(actual, vec![
        (tdir.mkpath("."), op::WRITE), // parent directory gets modified
        (tdir.mkpath("dir1"), op::WRITE), // parent directory gets modified
        (tdir.mkpath("dir2"), op::CREATE),
        (tdir.mkpath("file1"), op::CREATE),
    ]);
}

#[test]
fn poll_watch_file() {
    let tdir = TempDir::new("temp_dir").expect("failed to create temporary directory");

    tdir.create_all(vec![
        "file1",
    ]);

    let (tx, rx) = mpsc::channel();
    let mut watcher = PollWatcher::with_delay(tx, 50).expect("failed to create poll watcher");
    watcher.watch(tdir.mkpath("file1"), RecursiveMode::Recursive).expect("failed to watch directory");

    sleep(1100); // PollWatcher has only a resolution of 1 second

    tdir.write("file1");
    tdir.create("file2");

    assert_eq!(recv_events(&rx), vec![
        (tdir.mkpath("file1"), op::WRITE)
    ]);
}
