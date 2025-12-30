@echo off
REM Switch code page to UTF-8 to prevent encoding issues
chcp 65001 >nul
setlocal

REM ========================================================
REM  1. Run Tests
REM ========================================================
echo.
echo [INFO] Starting Cargo Nextest (Profile: CI)...
echo --------------------------------------------------------

REM Run the command
cargo nextest run --workspace --profile ci

REM Check exit code
if %ERRORLEVEL% NEQ 0 (
    echo.
    echo --------------------------------------------------------
    echo [ERROR] Tests failed!
    echo --------------------------------------------------------
    exit /b %ERRORLEVEL%
)

REM ========================================================
REM  2. Check Report
REM ========================================================
echo.
echo --------------------------------------------------------

REM Check if junit.xml exists in the current directory
if exist "%CD%\target\nextest\ci\junit.xml" (
    echo [SUCCESS] Test Report Generated!
    echo.
    echo    File: junit.xml
    echo    Path: %CD%\target\nextest\ci\junit.xml
    echo.
) else (
    echo [WARNING] Tests passed, but 'junit.xml' was NOT found in the root directory.
    echo           Please check .config/nextest.toml [profile.ci.junit] path setting.
)

endlocal