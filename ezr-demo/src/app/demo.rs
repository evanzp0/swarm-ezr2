use std::time::{Duration, Instant};

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn mk(
    id: u32,
    name: &str,
    protocol: Protocol,
    url: &str,
    path: &str,
    total: u64,
    state: TaskState,
    done: u64,
    conns: usize,
    base_speed: f64,
    created: &str,
) -> Task {
    let piece = chunk_size(protocol);
    let (mut connections, chunk_done) = make_chunk_conns(total, done, conns, piece, id as u64);
    if state == TaskState::Completed {
        connections.clear();
    }

    if state == TaskState::Downloading {
        let per = base_speed / connections.len().max(1) as f64;
        for c in &mut connections {
            c.speed = per;
        }
    }
    let downloaded = done.min(total);
    Task {
        id,
        name: name.to_string(),
        protocol,
        url: url.to_string(),
        save_path: path.to_string(),
        total,
        downloaded,
        speed: 0.0,
        base_speed,
        resumable: true,
        concurrency: conns,
        connections,
        chunk_done,
        seeders: 0,
        peers: 0,
        upload_speed: 0.0,
        uploaded: 0,
        seed_left: 0.0,
        post_process: false,
        post_started: None,
        retries: 0,
        max_retries: 5,
        fail_count: 0,
        made_progress: false,
        retry_in: None,
        fail_kind: None,
        flaky: false,
        verify_fail_once: false,
        precheck_fail_once: false,
        running_since: None,
        state,
        start_delay: 2.0,
        queued_since: None,
        has_slot: matches!(state, TaskState::Downloading),
        verify_started: None,
        error: None,
        checksum: None,
        verify_ok: None,
        proxy: ProxyChoice::Direct,
        created: created.to_string(),
    }
}

pub(super) fn demo_tasks() -> Vec<Task> {
    let now = Instant::now();
    let mut v = vec![
        mk(
            1,
            "ubuntu-24.04.2-desktop-amd64.iso",
            Protocol::Https,
            "https://releases.ubuntu.com/24.04/ubuntu-24.04.2-desktop-amd64.iso",
            "/srv/downloads/ubuntu-24.04.2-desktop-amd64.iso",
            6_210_000_000,
            TaskState::Downloading,
            2_810_000_000,
            8,
            14_500_000.0,
            "2026-02-10 09:32",
        ),

        mk(
            2,
            "media-assets-2026-pack.zip",
            Protocol::Http,
            "http://cdn.example-media.net/get.php?file=media-assets-2026-pack.zip",
            "/srv/downloads/media-assets-2026-pack.zip",
            812_000_000,
            TaskState::Downloading,
            316_000_000,
            1,
            2_400_000.0,
            "2026-02-10 10:26",
        ),
        mk(
            3,
            "archlinux-x86_64-2026.02.01.iso",
            Protocol::Bt,
            "magnet:?xt=urn:btih:9f86d081884c7d659a2feaa0c55ad015a3bf4f1b&dn=archlinux",
            "/srv/downloads/archlinux-x86_64-2026.02.01.iso",
            4_320_000_000,
            TaskState::Seeding,
            4_320_000_000,
            1,
            0.0,
            "2026-02-08 21:04",
        ),
        mk(
            4,
            "rust-toolchain-nightly.tar.zst",
            Protocol::Https,
            "https://static.rust-lang.org/dist/2026-02-09/rust-nightly-x86_64-unknown-linux-gnu.tar.zst",
            "/srv/downloads/rust-nightly.tar.zst",
            1_240_000_000,

            TaskState::Queued,
            421_000_000,
            4,
            5_800_000.0,
            "2026-02-10 10:02",
        ),
        mk(
            5,
            "blender-4.5-linux-x64.tar.xz",
            Protocol::Http,
            "https://download.blender.org/release/Blender4.5/blender-4.5-linux-x64.tar.xz",
            "/srv/downloads/blender-4.5.tar.xz",
            328_000_000,
            TaskState::Paused,
            123_000_000,
            4,
            8_600_000.0,
            "2026-02-10 08:47",
        ),
        mk(
            6,
            "llama3-8b-instruct-q4_k_m.gguf",
            Protocol::Https,
            "https://huggingface.co/meta-llama/Meta-Llama-3-8B-Instruct/resolve/main/llama3-8b-q4_k_m.gguf",
            "/srv/downloads/llama3-8b-q4_k_m.gguf",
            4_920_000_000,
            TaskState::Downloading,
            601_000_000,
            16,
            9_200_000.0,
            "2026-02-10 09:58",
        ),
        mk(
            7,
            "imagenet-mini-dataset.zip",
            Protocol::Http,
            "http://datasets.example-mirror.com/vision/imagenet-mini.zip",
            "/srv/downloads/imagenet-mini.zip",
            2_260_000_000,
            TaskState::Queued,
            0,
            8,
            6_000_000.0,
            "2026-02-10 10:15",
        ),
        mk(
            8,
            "ffmpeg-master-amd64.AppImage",
            Protocol::Https,
            "https://github.com/eugeneware/ffmpeg-static/releases/latest/ffmpeg-amd64",
            "/srv/downloads/ffmpeg-master-amd64.AppImage",
            96_400_000,
            TaskState::Completed,
            96_400_000,
            1,
            0.0,
            "2026-02-09 14:20",
        ),
        mk(
            9,
            "godot-4.4-stable-linux.tar.gz",
            Protocol::Http,
            "https://github.com/godotengine/godot/releases/download/4.4-stable/godot-4.4-stable-linux.tar.gz",
            "/srv/downloads/godot-4.4.tar.gz",
            61_800_000,
            TaskState::Completed,
            61_800_000,
            1,
            0.0,
            "2026-02-09 16:02",
        ),
        mk(
            10,
            "win11-24h2-x64.iso",
            Protocol::Https,
            "https://dl.example-microsoft.com/software-download/win11-24h2-x64.iso",
            "/srv/downloads/win11-24h2-x64.iso",
            5_710_000_000,
            TaskState::Failed,
            2_160_000_000,
            8,
            11_000_000.0,
            "2026-02-10 07:12",
        ),

        mk(
            12,
            "tensorflow-cuda-12.8.whl",
            Protocol::Http,
            "http://pypi.example-mirror.org/packages/tf/tensorflow-cuda-12.8-cp312.whl",
            "/srv/downloads/tensorflow-cuda-12.8.whl",
            712_000_000,
            TaskState::Queued,
            0,
            4,
            5_000_000.0,
            "2026-02-10 10:20",
        ),
        mk(
            11,
            "gpt4all-models-bundle.bin",
            Protocol::Https,
            "https://gpt4all.example-models.io/models/gpt4all-models-bundle.bin",
            "/srv/downloads/gpt4all-models-bundle.bin",
            3_340_000_000,
            TaskState::Verifying,
            3_340_000_000,
            8,
            0.0,
            "2026-02-10 10:18",
        ),
        mk(
            13,
            "neovim-0.11.2-linux-x86_64.tar.gz",
            Protocol::Https,
            "https://github.com/neovim/neovim/releases/download/v0.11.2/neovim-0.11.2-linux-x86_64.tar.gz",
            "/srv/downloads/neovim-0.11.2-linux-x86_64.tar.gz",
            23_400_000,
            TaskState::PostProcessing,
            23_400_000,
            2,
            0.0,
            "2026-02-10 10:21",
        ),
        mk(
            14,
            "sintel-4k-2160p-x265.mkv",
            Protocol::Bt,
            "magnet:?xt=urn:btih:c12fe1c06bba254a9dc9f519b335aa7c1366a6b6&dn=sintel-4k",
            "/srv/downloads/sintel-4k-2160p-x265.mkv",
            4_190_000_000,
            TaskState::Downloading,
            1_280_000_000,
            12,
            2_800_000.0,
            "2026-02-10 09:47",
        ),
    ];

    v[1].resumable = false;

    v[0].proxy = ProxyChoice::Named("办公网代理".to_string());

    v[2].seeders = 12;
    v[2].peers = 35;
    v[2].upload_speed = 1_900_000.0;
    v[2].uploaded = 1_712_000_000;
    v[2].seed_left = 1_800.0;
    v[2].verify_ok = Some(true);

    v[2].checksum = Some(Checksum {
        algo: "SHA-1",
        value: "a9993e364706816aba3e25717850c26c9cd0d89d".to_string(),
    });

    v[1].checksum = Some(Checksum {
        algo: "SHA-224",
        value: "d14a028c2a3a2bc9476102bb288234c415a2b01f828ea62ac5b3e42f".to_string(),
    });

    v[13].seeders = 8;
    v[13].peers = 23;
    v[13].upload_speed = 420_000.0;
    v[13].uploaded = 96_000_000;

    v[9].error = Some(fail_case(1).0.to_string());
    v[9].retries = 1;
    v[9].fail_count = 1;
    v[9].retry_in = Some(8.0);
    v[9].fail_kind = Some(FailKind::Transient);
    v[9].flaky = true;

    v[11].verify_started = Some(now + Duration::from_secs(8));
    v[11].verify_fail_once = true;
    v[11].checksum = Some(Checksum {
        algo: "SHA-256",
        value: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string(),
    });

    v[10].precheck_fail_once = true;

    v[12].post_started = Some(now - Duration::from_millis(1200));
    v[12].verify_ok = Some(true);
    v[12].checksum = Some(Checksum {
        algo: "SHA-512",
        value: "cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce".to_string()
            + "47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e",
    });
    v[12].post_process = true;

    v[3].queued_since = Some(now);
    v[6].queued_since = Some(now);
    v[10].queued_since = Some(now);

    v[2].post_process = true;
    v[3].post_process = true;
    v[6].post_process = true;
    v[11].post_process = true;

    v[8].verify_ok = Some(true);
    v[8].checksum = Some(Checksum {
        algo: "MD5",
        value: "d41d8cd98f00b204e9800998ecf8427e".to_string(),
    });

    v
}
