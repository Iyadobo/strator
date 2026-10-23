@echo off
cd /d "%~dp0"
if exist "%~dp0.local\bin\strator.cmd" goto installedcmd
if exist "%~dp0.local\bin\strator.exe" goto installed
if not exist "%~dp0target\release\strator.exe" goto build
"%~dp0target\release\strator.exe" %*
exit /b %errorlevel%
:build
cargo run -p strator-launcher --release --locked -- %*
exit /b %errorlevel%
:installed
"%~dp0.local\bin\strator.exe" %*
exit /b %errorlevel%
:installedcmd
call "%~dp0.local\bin\strator.cmd" %*
exit /b %errorlevel%
