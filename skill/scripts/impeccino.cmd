@echo off
setlocal
rem Impeccino launcher (Windows). Runs bin\windows-<arch>\impeccino.exe next
rem to this file, else a cached or freshly downloaded engine binary.
rem
rem Structure notes (this file is exercised by dry parsing and string-level
rem tests, not yet on a real Windows machine):
rem - No multi-line parenthesized blocks: cmd expands %var% at block parse
rem   time, which made the old download path read back empty %url%/%cached%.
rem   Linear goto flow keeps every expansion on its own line, and avoids
rem   delayed expansion eating ! characters in user arguments.
rem - The unversioned user binary and the PATH candidate are validated with
rem   the engine-probe handshake (see :probe) so the retired 3.x npm CLI,
rem   whose bin is also named impeccino, is never exec'd. IMPECCINO_BIN,
rem   the sibling binary, and the version-pinned cache stay trusted.
rem - Downloads are verified against the .sha256 sidecar via certutil and
rem   fail closed: a missing sidecar or hash tool refuses the download. On
rem   ARM64 the arm64 asset is tried first and the x64 asset is the
rem   fallback (Windows on ARM runs x64 binaries).
if not defined IMPECCINO_SKILL_DIR set "IMPECCINO_SKILL_DIR=%~dp0.."
if not defined IMPECCINO_SELF set "IMPECCINO_SELF=%~f0"
set "arch=x64"
if /I "%PROCESSOR_ARCHITECTURE%"=="ARM64" set "arch=arm64"

if not defined IMPECCINO_BIN goto no_env_bin
if not exist "%IMPECCINO_BIN%" goto no_env_bin
set "run=%IMPECCINO_BIN%"
goto run
:no_env_bin

set "bin=%~dp0bin\windows-%arch%\impeccino.exe"
if not exist "%bin%" goto no_sibling
set "run=%bin%"
goto run
:no_sibling

set "home_bin=%USERPROFILE%\.impeccino\bin\impeccino.exe"
if not exist "%home_bin%" goto no_home_bin
if defined IMPECCINO_LAUNCHER_PROBE goto no_home_bin
call :probe "%home_bin%"
if not "%probe_ok%"=="1" goto no_home_bin
set "run=%home_bin%"
goto run
:no_home_bin

set "version="
if exist "%~dp0VERSION" set /p version=<"%~dp0VERSION"
if not defined IMPECCINO_HOME set "IMPECCINO_HOME=%USERPROFILE%\.impeccino"
set "cached=%IMPECCINO_HOME%\bin\%version%\impeccino.exe"
if not defined version goto no_cache
if not exist "%cached%" goto no_cache
set "run=%cached%"
goto run
:no_cache

if defined IMPECCINO_LAUNCHER_PROBE goto download
where impeccino >nul 2>nul
if errorlevel 1 goto download
call :probe impeccino
if not "%probe_ok%"=="1" goto download
impeccino %*
exit /b

:download
rem Last resort: fetch this version's binary from the release channel into
rem the version-pinned user cache, verify it, then run it. Never inside
rem another launcher's probe: fail fast and quiet instead.
if defined IMPECCINO_LAUNCHER_PROBE exit /b 127
if not defined version goto fail
where curl.exe >nul 2>nul
if errorlevel 1 goto curl_missing
if not defined IMPECCINO_DOWNLOAD_BASE set "IMPECCINO_DOWNLOAD_BASE=https://github.com/sebastian-software/impeccino/releases/download"
if exist "%IMPECCINO_HOME%\bin\%version%\" goto cache_ready
mkdir "%IMPECCINO_HOME%\bin\%version%" >nul 2>nul
if errorlevel 1 goto cache_directory_failed
:cache_ready
rem Check the staging file too: an existing directory may be read-only.
rem Redirection failures do not reliably update ERRORLEVEL in cmd.exe;
rem branch on the command's failure directly. Never treat a directory as a
rem staging file (later del cleanup would prompt to delete its contents).
if exist "%cached%.part\" goto cache_write_failed
(type nul >"%cached%.part") 2>nul || goto cache_write_failed
set "asset=impeccino-windows-%arch%.exe"
set "url=%IMPECCINO_DOWNLOAD_BASE%/engine-v%version%/%asset%"
curl.exe -fsSL -o "%cached%.part" "%url%" >nul 2>nul
if not errorlevel 1 goto verify
if not "%arch%"=="arm64" goto download_failed
set "asset=impeccino-windows-x64.exe"
set "url=%IMPECCINO_DOWNLOAD_BASE%/engine-v%version%/%asset%"
curl.exe -fsSL -o "%cached%.part" "%url%" >nul 2>nul
if errorlevel 1 goto download_failed

:verify
call :check_download
if errorlevel 1 exit /b 127
rem Mirrors the sh launcher and fails closed: a freshly downloaded binary
rem runs only after verifying against its .sha256 sidecar. A sidecar that
rem cannot be fetched, or an empty certutil result, refuses the download
rem instead of running an unverified binary.
curl.exe -fsSL -o "%cached%.sha256" "%url%.sha256" >nul 2>nul
if errorlevel 1 goto verify_refuse
set "expected="
set /p expected=<"%cached%.sha256"
for /f "tokens=1" %%h in ("%expected%") do set "expected=%%h"
call :check_download
if errorlevel 1 exit /b 127
set "actual="
rem Reuse the sidecar staging file after reading expected. Check certutil's
rem status before parsing: its error text on stdout is not a digest.
certutil -hashfile "%cached%.part" SHA256 >"%cached%.sha256" 2>nul
if errorlevel 1 goto verify_refuse
call :check_download
if errorlevel 1 exit /b 127
for /f "usebackq skip=1 delims=" %%h in ("%cached%.sha256") do if not defined actual set "actual=%%h"
del "%cached%.sha256" >nul 2>nul
if not defined expected goto verify_refuse
if not defined actual goto verify_refuse
set "actual=%actual: =%"
if /I "%actual%"=="%expected%" goto place
del "%cached%.part" >nul 2>nul
echo impeccino: checksum mismatch downloading %url% 1>&2
exit /b 127

:verify_refuse
call :check_download
if errorlevel 1 exit /b 127
del "%cached%.part" >nul 2>nul
del "%cached%.sha256" >nul 2>nul
echo impeccino: cannot verify %url% against %url%.sha256; refusing the unverified download 1>&2
exit /b 127

:check_download
set "download_file=%~1"
if not defined download_file set "download_file=%cached%.part"
if not exist "%download_file%" goto download_missing
for %%f in ("%download_file%") do if %%~zf==0 goto download_empty
exit /b 0

:download_missing
del "%cached%.sha256" >nul 2>nul
echo impeccino: download completed but the file was removed before execution: %url%; check your antivirus quarantine or logs. Refusing to continue; do not disable protection. 1>&2
exit /b 127

:download_empty
del "%download_file%" >nul 2>nul
del "%cached%.sha256" >nul 2>nul
echo impeccino: downloaded file is empty: %url%; refusing the unverified download 1>&2
exit /b 127

:place
call :check_download
if errorlevel 1 exit /b 127
move /y "%cached%.part" "%cached%" >nul 2>nul
if errorlevel 1 goto place_failed
call :check_download "%cached%"
if errorlevel 1 exit /b 127
set "run=%cached%"
goto run

:place_failed
call :check_download
if errorlevel 1 exit /b 127
del "%cached%.part" >nul 2>nul
echo impeccino: could not cache the verified download: %url% 1>&2
exit /b 127

:run
"%run%" %*
exit /b

:probe
rem Sets probe_ok=1 when %1 answers the engine handshake: prints
rem "impeccino-engine <version>" and exits 0. The 3.x npm CLI answers any
rem unknown verb with "Unknown command", exit 1, so it never passes.
set "probe_ok="
set "probe_tmp=%TEMP%\impeccino-probe-%RANDOM%%RANDOM%.txt"
set "IMPECCINO_LAUNCHER_PROBE=1"
"%~1" engine-probe >"%probe_tmp%" 2>nul
set "probe_err=%ERRORLEVEL%"
set "IMPECCINO_LAUNCHER_PROBE="
if not "%probe_err%"=="0" goto probe_done
findstr /b /c:"impeccino-engine" "%probe_tmp%" >nul 2>nul
if not errorlevel 1 set "probe_ok=1"
:probe_done
del "%probe_tmp%" >nul 2>nul
exit /b 0

:cache_directory_failed
echo impeccino: engine %version% is not installed; cannot create cache directory: "%IMPECCINO_HOME%\bin\%version%" 1>&2
goto setup_failed

:cache_write_failed
echo impeccino: engine %version% is not installed; cannot write to cache directory: "%IMPECCINO_HOME%\bin\%version%" 1>&2
goto setup_failed

:curl_missing
echo impeccino: cannot download engine %version%; curl.exe is unavailable. 1>&2
goto setup_failed

:download_failed
del "%cached%.part" >nul 2>nul
echo impeccino: could not download engine %version% from %url%; check network access and the release URL. 1>&2

:setup_failed
echo Engine %version% setup needs network access and write permission to "%IMPECCINO_HOME%\bin\%version%". 1>&2
echo Run this launcher ("%~f0") with engine-probe in a terminal that has those permissions, then retry the original command. 1>&2
echo Alternatively, set IMPECCINO_HOME to a writable cache location, or IMPECCINO_BIN to a preinstalled engine binary. 1>&2
exit /b 127

:fail
del "%cached%.part" >nul 2>nul
echo impeccino: no engine binary found (looked in %bin%, %cached%, PATH). 1>&2
echo Download impeccino-windows-%arch%.exe from https://github.com/sebastian-software/impeccino/releases (tag engine-v%version%) and save it as %cached%, or set IMPECCINO_BIN to a preinstalled engine binary. Docs: https://impeccable.style 1>&2
exit /b 127
