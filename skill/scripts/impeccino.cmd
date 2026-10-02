@echo off
setlocal
rem Impeccino launcher (Windows). Runs a version-matched sibling or cache
rem binary, else downloads the skill's pinned engine into IMPECCINO_HOME.
rem
rem Keep this as straight-line goto flow: cmd expands %%var%% inside a
rem parenthesized block at parse time, and delayed expansion corrupts ! args.
rem Automatic binaries must answer the exact engine/version probe. An explicit
rem IMPECCINO_BIN is a developer override and is used without version pinning.
rem Downloads use unique temporary names and certutil verifies the skill pin.
if not defined IMPECCINO_SKILL_DIR set "IMPECCINO_SKILL_DIR=%~dp0.."
if not defined IMPECCINO_SELF set "IMPECCINO_SELF=%~f0"
if defined IMPECCINO_LAUNCHER_PROBE exit /b 127
set "arch=x64"
if /I "%PROCESSOR_ARCHITECTURE%"=="ARM64" set "arch=arm64"
set "version="
if exist "%~dp0VERSION" set /p version=<"%~dp0VERSION"
if not defined IMPECCINO_HOME set "IMPECCINO_HOME=%USERPROFILE%\.impeccino"
set "cache_dir=%IMPECCINO_HOME%\bin\%version%"
set "cached=%cache_dir%\impeccino.exe"
set "failure_marker=%cache_dir%\.impeccino-download-failed"

if not defined IMPECCINO_BIN goto no_env_bin
if not exist "%IMPECCINO_BIN%" goto invalid_env_bin
if exist "%IMPECCINO_BIN%\NUL" goto invalid_env_bin
set "run=%IMPECCINO_BIN%"
goto run
:invalid_env_bin
echo impeccino: IMPECCINO_BIN points to a missing or unusable executable: "%IMPECCINO_BIN%" 1>&2
exit /b 127
:no_env_bin

if not defined version goto sibling_done
set "bin=%~dp0bin\windows-%arch%\impeccino.exe"
if not exist "%bin%" goto sibling_done
call :probe "%bin%" "%version%"
if not "%probe_ok%"=="1" goto sibling_done
set "run=%bin%"
goto run
:sibling_done

if not defined version goto no_cache
if not exist "%cached%" goto no_cache
call :probe "%cached%" "%version%"
if not "%probe_ok%"=="1" goto no_cache
set "run=%cached%"
goto run
:no_cache

:download
rem Never fetch a version the skill does not pin. A recent network failure
rem gets a five-minute cooldown; healthy concurrent downloads use unique files.
if not defined version goto fail
if not defined IMPECCINO_DOWNLOAD_BASE set "IMPECCINO_DOWNLOAD_BASE=https://github.com/sebastian-software/impeccino/releases/download"
set "powershell=%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe"
if not exist "%failure_marker%" goto cooldown_done
call :cooldown_active
if "%cooldown_active%"=="1" goto cooldown_refused
:cooldown_done
if exist "%cache_dir%\" goto cache_ready
mkdir "%cache_dir%" >nul 2>nul
if errorlevel 1 goto cache_directory_failed
:cache_ready
set "stage=%cache_dir%\impeccino-%RANDOM%%RANDOM%.part"
if exist "%stage%" goto cache_ready
set "hash_tmp=%stage%.hash"
if exist "%hash_tmp%" goto cache_ready
if exist "%stage%\" goto cache_write_failed
(type nul >"%stage%") 2>nul || goto cache_write_failed
set "asset=impeccino-windows-%arch%.exe"
set "url=%IMPECCINO_DOWNLOAD_BASE%/engine-v%version%/%asset%"
findstr /c:"  engine-v%version%/" "%~dp0engine.sha256" >nul 2>nul
if errorlevel 1 goto no_pin
curl.exe -fsSL --connect-timeout 5 --max-time 60 -o "%stage%" "%url%" >nul 2>nul
if not errorlevel 1 goto verify
if not "%arch%"=="arm64" goto download_failed
set "asset=impeccino-windows-x64.exe"
set "url=%IMPECCINO_DOWNLOAD_BASE%/engine-v%version%/%asset%"
curl.exe -fsSL --connect-timeout 5 --max-time 60 -o "%stage%" "%url%" >nul 2>nul
if errorlevel 1 goto download_failed

:verify
call :check_download "%stage%"
if errorlevel 1 exit /b 127
set "expected="
if exist "%~dp0engine.sha256" for /f "usebackq tokens=1,2" %%a in ("%~dp0engine.sha256") do if "%%b"=="engine-v%version%/%asset%" if not defined expected set "expected=%%a"
if not defined expected goto no_pin_downloaded
call :check_download "%stage%"
if errorlevel 1 exit /b 127
set "actual="
certutil -hashfile "%stage%" SHA256 >"%hash_tmp%" 2>nul
if errorlevel 1 goto verify_refuse
call :check_download "%stage%"
if errorlevel 1 exit /b 127
for /f "usebackq skip=1 delims=" %%h in ("%hash_tmp%") do if not defined actual set "actual=%%h"
del "%hash_tmp%" >nul 2>nul
if not defined expected goto verify_refuse
if not defined actual goto verify_refuse
set "actual=%actual: =%"
if /I "%actual%"=="%expected%" goto place
del "%stage%" >nul 2>nul
echo impeccino: checksum mismatch downloading %url% (pinned in "%~dp0engine.sha256") 1>&2
exit /b 127

:verify_refuse
call :check_download "%stage%"
if errorlevel 1 exit /b 127
del "%stage%" >nul 2>nul
del "%hash_tmp%" >nul 2>nul
echo impeccino: cannot hash %url% with certutil; refusing the unverified download 1>&2
exit /b 127

:no_pin_downloaded
call :check_download "%stage%"
if errorlevel 1 exit /b 127
del "%stage%" >nul 2>nul

:no_pin
del "%stage%" >nul 2>nul
echo impeccino: no digest pinned for engine-v%version%/%asset% in "%~dp0engine.sha256"; refusing to download an unverified engine. Use a released skill version, or build the engine and set IMPECCINO_BIN. 1>&2
exit /b 127

:check_download
set "download_file=%~1"
if not defined download_file set "download_file=%stage%"
if not exist "%download_file%" goto download_missing
for %%f in ("%download_file%") do if %%~zf==0 goto download_empty
exit /b 0

:download_missing
del "%hash_tmp%" >nul 2>nul
echo impeccino: download completed but the file was removed before execution: %url%; check your antivirus quarantine or logs. Refusing to continue; do not disable protection. 1>&2
exit /b 127

:download_empty
del "%download_file%" >nul 2>nul
del "%hash_tmp%" >nul 2>nul
echo impeccino: downloaded file is empty: %url%; refusing the unverified download 1>&2
exit /b 127

:place
call :check_download "%stage%"
if errorlevel 1 exit /b 127
move /y "%stage%" "%cached%" >nul 2>nul
if errorlevel 1 goto place_failed
call :check_download "%cached%"
if errorlevel 1 exit /b 127
del "%failure_marker%" >nul 2>nul
set "run=%cached%"
goto run

:place_failed
call :check_download "%stage%"
if errorlevel 1 exit /b 127
del "%stage%" >nul 2>nul
echo impeccino: could not cache the verified download: %url% 1>&2
exit /b 127

:run
"%run%" %*
exit /b

:probe
rem Automatic sibling/cache binaries must exit successfully and emit exactly
rem one line: "impeccino-engine <pinned version>".
set "probe_ok=0"
set "probe_tmp=%TEMP%\impeccino-probe-%RANDOM%%RANDOM%.txt"
set "IMPECCINO_LAUNCHER_PROBE=1"
"%~1" engine-probe >"%probe_tmp%" 2>nul
set "probe_status=%ERRORLEVEL%"
set "IMPECCINO_LAUNCHER_PROBE="
if not "%probe_status%"=="0" goto probe_done
set "probe_lines=0"
for /f %%c in ('find /v /c "" ^< "%probe_tmp%"') do set "probe_lines=%%c"
if not "%probe_lines%"=="1" goto probe_done
findstr /x /c:"impeccino-engine %~2" "%probe_tmp%" >nul 2>nul
if errorlevel 1 goto probe_done
set "probe_ok=1"
:probe_done
del "%probe_tmp%" >nul 2>nul
exit /b 0

:cooldown_active
set "cooldown_active=0"
set "IMPECCINO_COOLDOWN_FILE=%failure_marker%"
"%powershell%" -NoProfile -NonInteractive -Command "$p=$env:IMPECCINO_COOLDOWN_FILE; $until=0L; $raw=Get-Content -Raw -LiteralPath $p -ErrorAction SilentlyContinue; if ($null -eq $raw) { Remove-Item -LiteralPath $p -Force -ErrorAction SilentlyContinue; exit 0 }; if ([long]::TryParse($raw.Trim(), [ref]$until) -and $until -gt [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()) { exit 2 }; Remove-Item -LiteralPath $p -Force -ErrorAction SilentlyContinue; exit 0" <nul >nul 2>nul
set "cooldown_status=%ERRORLEVEL%"
set "IMPECCINO_COOLDOWN_FILE="
if "%cooldown_status%"=="2" set "cooldown_active=1"
exit /b 0

:record_failure
set "IMPECCINO_COOLDOWN_FILE=%failure_marker%"
"%powershell%" -NoProfile -NonInteractive -Command "$p=$env:IMPECCINO_COOLDOWN_FILE; [IO.File]::WriteAllText($p, [string](([DateTimeOffset]::UtcNow.ToUnixTimeSeconds())+300))" <nul >nul 2>nul
set "IMPECCINO_COOLDOWN_FILE="
exit /b 0

:cache_directory_failed
echo impeccino: engine %version% is not installed; cannot create cache directory: "%cache_dir%" 1>&2
goto setup_failed

:cache_write_failed
echo impeccino: engine %version% is not installed; cannot write to cache directory: "%cache_dir%" 1>&2
goto setup_failed

:curl_missing
echo impeccino: cannot download engine %version%; curl.exe is unavailable. 1>&2
goto setup_failed

:download_failed
del "%stage%" >nul 2>nul
call :record_failure
echo impeccino: could not download engine %version% from %url%; check network access and the release URL. 1>&2
goto setup_failed

:cooldown_refused
echo impeccino: download of engine %version% recently failed; retry after the five-minute cooldown. 1>&2
goto setup_failed

:setup_failed
echo Engine %version% setup needs network access and write permission to "%cache_dir%". 1>&2
echo Run this launcher ("%~f0") with engine-probe in a terminal that has those permissions, then retry the original command. 1>&2
echo Alternatively, set IMPECCINO_HOME to a writable cache location, or IMPECCINO_BIN to a preinstalled engine binary. 1>&2
exit /b 127

:fail
echo impeccino: no engine binary for Windows-%arch% found (looked in %bin% and %cached%). 1>&2
echo Download impeccino-windows-%arch%.exe from https://github.com/sebastian-software/impeccino/releases (tag engine-v%version%) into "%cached%", or set IMPECCINO_BIN to a preinstalled engine binary. 1>&2
exit /b 127
