# Audit dan port bugfix upstream Concat — 24 Okt 2026

## Batas audit

- Fork: `pujosety/Lunola-Mova`, branch `feat/lunola-mova`; checkpoint sebelum port: `a0d3cc6f`.
- Upstream yang diaudit: `jub0t/Concat`, `origin/main` `1d04d147`; common base tercatat `0660c019`.
- Port manual yang menjaga perubahan Lunola. Tidak cherry-pick massal, tidak reset working tree.
- Media clipboard hanya menyalin metadata dan path. Tidak ada file media pengguna yang disalin atau diunggah.

## Hasil dan status

| Perilaku upstream | Port Lunola | Bukti saat ini |
|---|---|---|
| `1d04d147` — copy/paste/duplicate mempertahankan properti clip | `MatchClip` internal menyalin model clip lengkap sambil menjaga identitas, lane, media lokal, waktu, trim, speed, dan hubungan tetangga. Jalur text/shape juga melalui perintah ini; penolakan edit properti membatalkan penempatan. | `concat-project`: 119 unit + 21 AV/timeline + 8 cutout + 7 ripple-trim lulus. Kontrak clipboard 7/7 lulus. |
| `1d04d147` — paste antarproyek membawa media yang dirujuk | Clipboard membawa record metadata; target memetakan ID lokal menurut path atau menambahkan record ber-ID baru. Path yang sama dengan kind berbeda ditolak; semua tujuan harus tersedia; kegagalan edit membatalkan satu gesture paste. `Clip::media_ids`/`remap_media_ids` mencakup tautan Enhance. | Tes rollback session 1/1 dan kontrak clipboard 7/7 lulus; GUI paste antarproyek belum diuji. |
| `73d8e9d4` — deduplikasi path Windows | `same_media_path` membandingkan huruf ASCII tanpa membedakan kapitalisasi dan menormalkan slash; path kosong tidak dianggap identitas file. | Regresi Windows awal merah, lalu tes duplikasi path lulus dalam suite `concat-project`. |
| `4b1b2b05` — preferensi rusak | Nilai valid dipulihkan, berkas malformed dikarantina, save memakai temp unik. | Tes Windows `malformed_settings_keep_good_values_and_quarantine_original` lulus 1/1. |
| `e2be568a` — shortcut saat mengetik/ada sheet | Guard text-input dan sheet mencakup shortcut langsung serta menu; Save tetap diizinkan. | Kontrak Q/W/shortcut 4/4 lulus; bukan pengujian keyboard GUI. |
| `b18c2fe5` + `a9b2df61` — DX12 compiler | FXC menjadi default di window dan compositor; `WGPU_DX12_COMPILER` tetap menjadi override. | Kontrak compiler 1/1 lulus; belum diuji di GPU DX12 runtime. |
| `e75af9b8` — urutan save | Ticket saat snapshot, serialisasi per folder proyek, temp unik `create_new`, flush, atomic replace dan retry singkat pada sharing violation Windows. Save lama yang datang terlambat dilewati. | `concat-host`: 86 unit + 11 export lulus; mencakup save terlambat, save konkuren, gagal replace dengan temp dibersihkan, dan rollback gesture paste. |
| `2abfcf7d` + bagian aman dari `91973407` — switch/close | Folder target diklaim dan dibuka/validasi sebelum proyek lama disimpan/ditutup. Target sibuk atau gagal dibuka tidak menyentuh sesi lama; jika save gagal, claim target dilepas dan sesi lama/lock dipertahankan. | Tes `concat-api` claim/rollback 2/2 dan kontrak lifecycle 4/4 lulus; belum ada uji UI runtime. |
| `3f6db386` + prasyarat `b5e6c086`/`e09d154f` — WARP/software adapter Windows | `platform.rs` deteksi WARP, melepas device bersama, memilih renderer Skia software, dan mengatur `CONCAT_GPU=software`; window adapter dan compositor menghormati mode software-only. | Kontrak software adapter 3/3 lulus; belum ada GUI/WARP runtime. Startup crash-loop recovery dari `b5e6c086` tidak dipindahkan. |
| `8ee4f93e` — VLC MP4 tanpa keyframe pertama di indeks | Seek bounded gagal → fallback seek ke awal; decode tetap membuang frame sampai target. Berlaku untuk seek biasa dan resume software. | Tes fixture MP4 semula merah dengan `Operation not permitted`; suite `concat-media` lulus 102/102 setelah port. |

## Batas yang belum selesai

- Belum menjalankan MSI, release, legal clearance, commit, push, atau tag. Status source/legal dari audit sebelumnya tetap `NOT_ASSESSED`; bundle bukti parsial bukan Corresponding Source lengkap.
- Tes GUI/keyboard, paste antarproyek nyata, dan WARP runtime **NOT RUN** sesuai consent gate; kontrak source bukan bukti runtime.
- Bagian UX `91973407` yang menawarkan pilihan eksplisit **“quit without saving”** belum dipindahkan. Implementasi Lunola memilih perilaku konservatif: pertahankan proyek dan window terbuka bila save gagal.
- `rustfmt --check` terfokus pada 14 file Rust port lulus. Pemeriksaan lebih luas masih menemukan format lama pada blok Cutout di `concat-project/src/model.rs` dan blok alpha di `concat-render/src/gpu.rs`; keduanya di luar hunk port dan tidak diformat ulang agar perubahan tak terkait tetap utuh.
- Windows `cargo check --locked --profile quick -p concat --tests --target x86_64-pc-windows-msvc -j 1` lulus. Suite penuh `cargo test --locked --profile quick -p concat --target x86_64-pc-windows-msvc -j 1` lulus: 102 tes library + 5 launcher, 0 gagal; doc-tests 0. Dua warning dead-code lama tetap di `concat-vision` (`generation`, `remove`).
