@echo off
rem Builds the two Windows versions of NapkinUML and copies them to dist\
rem Needs: rustup targets x86_64-pc-windows-msvc and aarch64-pc-windows-msvc,
rem and Visual Studio's C++ build tools for both x64 and ARM64.
cd /d "%~dp0"
cargo build --release --target x86_64-pc-windows-msvc || exit /b 1
cargo build --release --target aarch64-pc-windows-msvc || exit /b 1
if not exist dist mkdir dist
copy /y target\x86_64-pc-windows-msvc\release\NapkinUML.exe dist\NapkinUML-x64.exe >nul
copy /y target\aarch64-pc-windows-msvc\release\NapkinUML.exe dist\NapkinUML-arm64.exe >nul
echo Built dist\NapkinUML-x64.exe and dist\NapkinUML-arm64.exe
