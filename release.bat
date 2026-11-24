@echo off
setlocal EnableExtensions
cd /d "%~dp0"

set "REQ_VER="
set "AUTO_YES=0"
set "LOCAL_BUILD=0"
set "SKIP_TESTS=0"
set "INCLUDE_ALL=0"

:parse_args
if "%~1"=="" goto :args_done
set "ARG=%~1"
if /I "%ARG%"=="--help" goto :show_help
if /I "%ARG%"=="-h" goto :show_help
if "%ARG%"=="/?" goto :show_help
if /I "%ARG%"=="--yes" (
    set "AUTO_YES=1"
    shift
    goto :parse_args
)
if /I "%ARG%"=="-y" (
    set "AUTO_YES=1"
    shift
    goto :parse_args
)
if /I "%ARG%"=="--local-build" (
    set "LOCAL_BUILD=1"
    shift
    goto :parse_args
)
if /I "%ARG%"=="--skip-tests" (
    set "SKIP_TESTS=1"
    shift
    goto :parse_args
)
if /I "%ARG%"=="--include-all" (
    set "INCLUDE_ALL=1"
    shift
    goto :parse_args
)
if "%ARG:~0,1%"=="-" (
    echo Error: Unknown option "%ARG%"
    echo.
    goto :show_help_err
)
if not "%REQ_VER%"=="" (
    echo Error: Multiple version arguments specified: "%REQ_VER%" and "%ARG%"
    echo.
    goto :show_help_err
)
set "REQ_VER=%ARG%"
shift
goto :parse_args

:show_help
echo MLV Explorer Release Automation Script
echo.
echo Usage:
echo   release.bat [version] [options]
echo.
echo Arguments:
echo   version         Release version (e.g. 0.0.3 or v0.0.3). Prompts if omitted.
echo.
echo Options:
echo   --yes, -y       Non-interactive mode (accept defaults and auto-push tag).
echo   --local-build   Run scripts\package-release.ps1 before tagging.
echo   --skip-tests    Skip running cargo test --no-fail-fast before commit/tag.
echo   --include-all   Stage and commit all working tree changes (default commits
echo                   only Cargo.toml and Cargo.lock).
echo   --help, -h, /?  Show this help message and exit.
exit /b 0

:show_help_err
call :show_help
exit /b 1

:args_done
where git >nul 2>&1
if errorlevel 1 (
    echo Error: git was not found on PATH.
    goto :fail
)
where cargo >nul 2>&1
if errorlevel 1 (
    echo Error: cargo was not found on PATH.
    goto :fail
)

set "CURRENT_VER="
set "LATEST_TAG="
set "SUGGESTED_VER="
set "ENV_TMP=%TEMP%\mlv_rel_info_%RANDOM%.bat"

powershell -NoProfile -Command "$ErrorActionPreference = 'SilentlyContinue'; $toml = Get-Content 'Cargo.toml' -Raw; $cur = if ($toml -match '(?ms)\[package\].*?^version\s*=\s*\x22([^\x22]+)\x22') { $Matches[1] } else { '0.0.1' }; $tags = @(git tag -l); $max = [version]::new(0,0,0); $latestTag = '(none)'; $cleanCur = $cur -replace '-.*$',''; if ([version]::TryParse($cleanCur, [ref]$null)) { $max = [version]::Parse($cleanCur) }; foreach ($t in $tags) { $c = $t -replace '^[vV]',''; if ($c -match '^(\d+)\.(\d+)\.(\d+)$') { $v = [version]::Parse($c); if ($v -ge $max) { $max = $v; $latestTag = $t } } }; $next = '{0}.{1}.{2}' -f $max.Major, $max.Minor, ($max.Build + 1); $q = [char]34; Write-Output ('@set ' + $q + 'CURRENT_VER=' + $cur + $q); Write-Output ('@set ' + $q + 'LATEST_TAG=' + $latestTag + $q); Write-Output ('@set ' + $q + 'SUGGESTED_VER=' + $next + $q)" > "%ENV_TMP%"

if exist "%ENV_TMP%" (
    call "%ENV_TMP%"
    del /q "%ENV_TMP%"
)

if "%CURRENT_VER%"=="" set "CURRENT_VER=unknown"
if "%LATEST_TAG%"=="" set "LATEST_TAG=(none)"
if "%SUGGESTED_VER%"=="" set "SUGGESTED_VER=0.0.2"

echo ====================================================
echo  MLV Explorer - Release Pipeline Trigger
echo ====================================================
echo  Current Cargo.toml version : %CURRENT_VER%
echo  Latest git tag             : %LATEST_TAG%
echo  Suggested next version     : %SUGGESTED_VER%
echo ====================================================
echo.

if "%REQ_VER%"=="" (
    if "%AUTO_YES%"=="1" (
        set "REQ_VER=%SUGGESTED_VER%"
    ) else (
        set /p "REQ_VER=Enter release version [%SUGGESTED_VER%]: "
    )
)
if "%REQ_VER%"=="" set "REQ_VER=%SUGGESTED_VER%"

set "VERSION="
set "PWSH_REQ=%REQ_VER%"
set "VER_TMP=%TEMP%\mlv_rel_ver_%RANDOM%.bat"
powershell -NoProfile -Command "$v = $env:PWSH_REQ.Trim() -replace '^[vV]', ''; $q = [char]34; if ($v -match '^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$') { Write-Output ('@set ' + $q + 'VERSION=' + $v + $q) }" > "%VER_TMP%"
if exist "%VER_TMP%" (
    call "%VER_TMP%"
    del /q "%VER_TMP%"
)

if "%VERSION%"=="" (
    echo Error: Invalid semantic version "%REQ_VER%". Expected X.Y.Z or vX.Y.Z.
    goto :fail
)

set "TAG_NAME=v%VERSION%"
git rev-parse -q --verify "refs/tags/%TAG_NAME%" >nul 2>&1
if not errorlevel 1 (
    echo Error: Git tag "%TAG_NAME%" already exists locally. Choose a newer version.
    goto :fail
)
git rev-parse -q --verify "refs/tags/%VERSION%" >nul 2>&1
if not errorlevel 1 (
    echo Error: Git tag "%VERSION%" already exists locally. Choose a newer version.
    goto :fail
)

set "HAS_CHANGES=0"
for /f "delims=" %%S in ('git status --porcelain') do set "HAS_CHANGES=1"

if "%HAS_CHANGES%"=="1" (
    if "%INCLUDE_ALL%"=="0" (
        echo.
        echo Working tree has existing modified/untracked files:
        git status -s
        echo.
        if "%AUTO_YES%"=="0" (
            set "ANS_ALL="
            set /p "ANS_ALL=Include ALL uncommitted changes in release commit %TAG_NAME%? [y/N]: "
            if /I "%ANS_ALL%"=="y" set "INCLUDE_ALL=1"
            if /I "%ANS_ALL%"=="yes" set "INCLUDE_ALL=1"
        ) else (
            echo Note: --yes without --include-all will commit ONLY Cargo.toml and Cargo.lock.
        )
    )
)

echo.
echo [1/6] Updating Cargo.toml [package] version to %VERSION%...
set "PWSH_VER=%VERSION%"
powershell -NoProfile -Command "$p = (Resolve-Path 'Cargo.toml').Path; $enc = New-Object System.Text.UTF8Encoding($false); $txt = [System.IO.File]::ReadAllText($p, $enc); $re = [regex]::new('(?ms)(\[package\].*?^version\s*=\s*)\x22[^\x22]+\x22'); $q = [char]34; $out = $re.Replace($txt, ('${1}' + $q + $env:PWSH_VER + $q), 1); [System.IO.File]::WriteAllText($p, $out, $enc)"
if errorlevel 1 (
    echo Error: Failed to update Cargo.toml.
    goto :fail
)

set "CHECK_VER="
set "CHK_TMP=%TEMP%\mlv_rel_chk_%RANDOM%.bat"
powershell -NoProfile -Command "$t = Get-Content 'Cargo.toml' -Raw; $q = [char]34; if ($t -match '(?ms)\[package\].*?^version\s*=\s*\x22([^\x22]+)\x22') { Write-Output ('@set ' + $q + 'CHECK_VER=' + $Matches[1] + $q) }" > "%CHK_TMP%"
if exist "%CHK_TMP%" (
    call "%CHK_TMP%"
    del /q "%CHK_TMP%"
)
if not "%CHECK_VER%"=="%VERSION%" (
    echo Error: Verified Cargo.toml version is "%CHECK_VER%", expected "%VERSION%".
    goto :fail
)

echo [2/6] Updating Cargo.lock...
cargo update --workspace --offline >nul 2>&1
if errorlevel 1 (
    cargo metadata --format-version 1 >nul
    if errorlevel 1 (
        echo Error: Failed to update Cargo.lock.
        goto :fail
    )
)

if "%SKIP_TESTS%"=="0" (
    echo [3/6] Running test suite ^(cargo test --no-fail-fast^)...
    cargo test --no-fail-fast
    if errorlevel 1 (
        echo Error: Tests failed. Aborting before committing or tagging.
        goto :fail
    )
) else (
    echo [3/6] Skipping test suite ^(--skip-tests^).
)

if "%LOCAL_BUILD%"=="0" (
    if "%AUTO_YES%"=="0" (
        set "ANS_BUILD="
        set /p "ANS_BUILD=Build local Windows package + installer before tagging? [y/N]: "
        if /I "%ANS_BUILD%"=="y" set "LOCAL_BUILD=1"
        if /I "%ANS_BUILD%"=="yes" set "LOCAL_BUILD=1"
    )
)

if "%LOCAL_BUILD%"=="1" (
    echo [4/6] Running local packager ^(scripts\package-release.ps1^)...
    powershell -NoProfile -ExecutionPolicy Bypass -File "scripts\package-release.ps1"
    if errorlevel 1 (
        echo Error: Local package build failed. Aborting before committing or tagging.
        goto :fail
    )
) else (
    echo [4/6] Skipping local package build.
)

echo [5/6] Committing and creating annotated tag %TAG_NAME%...
if "%INCLUDE_ALL%"=="1" (
    git add -A
    if errorlevel 1 goto :fail
    git commit -m "chore(release): %TAG_NAME%"
    if errorlevel 1 goto :fail
) else (
    git add Cargo.toml Cargo.lock
    if errorlevel 1 goto :fail
    git diff --cached --quiet -- Cargo.toml Cargo.lock
    if errorlevel 1 (
        git commit -m "chore(release): %TAG_NAME%" -- Cargo.toml Cargo.lock
        if errorlevel 1 goto :fail
    ) else (
        echo Note: Cargo.toml and Cargo.lock already match %VERSION%; tagging current HEAD.
    )
)

git tag -a "%TAG_NAME%" -m "Release %TAG_NAME%"
if errorlevel 1 (
    echo Error: Failed to create git tag %TAG_NAME%.
    goto :fail
)

set "DO_PUSH=1"
if "%AUTO_YES%"=="0" (
    set "ANS_PUSH="
    set /p "ANS_PUSH=Push commit and tag %TAG_NAME% to origin to trigger release pipeline? [Y/n]: "
    if /I "%ANS_PUSH%"=="n" set "DO_PUSH=0"
    if /I "%ANS_PUSH%"=="no" set "DO_PUSH=0"
)

if "%DO_PUSH%"=="1" (
    echo [6/6] Pushing branch and tag %TAG_NAME% to origin...
    git push origin HEAD
    if errorlevel 1 (
        echo Error: Failed to push branch to origin.
        goto :fail
    )
    git push origin "%TAG_NAME%"
    if errorlevel 1 (
        echo Error: Failed to push tag %TAG_NAME% to origin.
        goto :fail
    )
    echo.
    echo ====================================================
    echo  Release %TAG_NAME% triggered successfully!
    echo  Pipeline : https://github.com/JavChz/mlvExplorer/actions
    echo  Release  : https://github.com/JavChz/mlvExplorer/releases/tag/%TAG_NAME%
    echo ====================================================
) else (
    echo.
    echo ====================================================
    echo  Local commit and tag %TAG_NAME% created ^(not pushed^).
    echo  Push manually when ready:
    echo    git push origin HEAD
    echo    git push origin %TAG_NAME%
    echo ====================================================
)

if "%AUTO_YES%"=="0" pause
exit /b 0

:fail
echo.
echo Release script aborted.
if "%AUTO_YES%"=="0" pause
exit /b 1
