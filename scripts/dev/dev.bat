@echo off
REM ServerKit Docker Dev Helper
REM Usage: scripts\dev\dev.bat [command]

REM Get project root (two levels up from this script)
set "SCRIPT_DIR=%~dp0"
set "PROJECT_ROOT=%SCRIPT_DIR%..\.."
pushd "%PROJECT_ROOT%"

if "%1"=="" goto help
if "%1"=="up" goto up
if "%1"=="down" goto down
if "%1"=="logs" goto logs
if "%1"=="shell" goto shell
if "%1"=="reset" goto reset
goto help

:up
echo Starting ServerKit (Docker)...
docker compose up --build -d
echo.
echo   Frontend: http://localhost:3847
echo   Backend:  http://localhost:5849
echo.
goto end

:down
docker compose down
goto end

:logs
docker compose logs -f
goto end

:shell
docker exec -it serverkit-backend /bin/bash
goto end

:reset
docker compose down -v
echo Run 'dev.bat up' to start fresh.
goto end

:help
echo.
echo Usage: scripts\dev\dev.bat [command]
echo.
echo   up      Start containers
echo   down    Stop containers
echo   logs    View logs
echo   shell   Shell into backend
echo   reset   Reset all data
echo.
goto end

:end
popd
