@echo off
setlocal
chcp 65001 >nul
set "SCRIPT_DIR=%~dp0"
set "INSTALLER=%SCRIPT_DIR%scripts\install.py"
set "LOCAL_UV=%SCRIPT_DIR%.installer\uv\uv.exe"
set "PYTHON_REQUEST=>=3.10,<3.15"
set "UV_EXE="

if not "%INSTPLOT_FORCE_UV_BOOTSTRAP%"=="1" (
    for /f "delims=" %%I in ('where uv 2^>nul') do if not defined UV_EXE set "UV_EXE=%%I"
)

if not defined UV_EXE (
    powershell -NoProfile -ExecutionPolicy Bypass -File "%SCRIPT_DIR%scripts\bootstrap_uv.ps1"
    if errorlevel 1 goto install_failed
    set "UV_EXE=%LOCAL_UV%"
)

"%UV_EXE%" run --no-project --python "%PYTHON_REQUEST%" "%INSTALLER%" --apply %*

:after_install
if errorlevel 1 goto install_failed

powershell -NoProfile -ExecutionPolicy Bypass -File "%SCRIPT_DIR%scripts\create_windows_shortcut.ps1"
if errorlevel 1 goto install_failed

if "%INSTPLOT_INSTALL_ONLY%"=="1" exit /b 0

call "%SCRIPT_DIR%run_instplot.bat"
exit /b %errorlevel%

:install_failed
echo Installation failed. See .install-logs in the project directory.
if not "%INSTPLOT_INSTALL_ONLY%"=="1" pause
exit /b 1
