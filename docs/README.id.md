# Lunola Mova

**Make Every Frame Move.**

Fork independen Concat, karya **Jareer (@jub0t) dan Concat contributors**,
dengan warna merek `#B65CF2`. Tidak disahkan atau didukung upstream.
Status: **draft pengembangan**, bukan pengumuman rilis.

Kode yang diwarisi mencakup editor Rust/Slint, timeline, keyframe, efek/judul,
impor/ekspor, transkripsi, sintesis suara, cutout, CLI, JSON-RPC dan gRPC feature-gated.
Semuanya memerlukan validasi runtime fork. MCP belum tersedia dalam upstream
dan ditunda, bukan fitur yang dihapus oleh fork. Satu model segmentasi tertanam;
model lain dapat memerlukan unduhan terpisah. Jangan menjanjikan codec,
performa atau privasi yang belum diuji.

Windows x64 adalah target awal; sumber installer tersedia tetapi tidak dinyatakan
sudah dibangun. Executable sasaran `lunola-mova.exe`, package/library `concat`.
Windows ARM64, macOS, Linux, Android dan iOS/iPadOS masih perlu validasi dan
pekerjaan publikasi. Belum ada situs unduh, kanal dukungan atau sertifikat Lunola
yang ditetapkan oleh draft ini.

- [README utama](../README.md) · [Kontribusi](../CONTRIBUTING.md)
- [Status signing](../CODE_SIGNING.md)
- [Checklist platform, source access dan legal staging](lunola/packaging-publication.md)
- [LICENSE](../LICENSE) · [Exceptions](../LICENSE-EXCEPTIONS.md)
- [Trademark upstream](../TRADEMARK.md) · [Notices](../THIRD_PARTY_NOTICES.md)

[jub0t/Concat](https://github.com/jub0t/Concat) adalah **upstream**, bukan rilis
atau dukungan resmi Lunola. Pertahankan copyright dan riwayat Git. Distribusi
binary memerlukan Corresponding Source yang cocok dan full licenses/notices
actual; link upstream saja tidak menyediakan source modifikasi fork.
Clearance legal dan hak artwork masih menjadi gate rilis.
