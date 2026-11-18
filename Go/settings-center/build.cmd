@echo off
setlocal
cd /d "%~dp0"
if defined DOTISLE_GO_BIN set "PATH=%DOTISLE_GO_BIN%;%PATH%"
if not defined GOPATH set "GOPATH=%USERPROFILE%\go"
for /f "tokens=2" %%V in ('findstr /b /c:"go " go.mod') do if not defined DOTISLE_GO_BIN if exist "%GOPATH%\pkg\mod\golang.org\toolchain@v0.0.1-go%%V.windows-amd64\bin\go.exe" set "PATH=%GOPATH%\pkg\mod\golang.org\toolchain@v0.0.1-go%%V.windows-amd64\bin;%PATH%"
if not defined GOCACHE set "GOCACHE=%TEMP%\DotIsle-go-build"
set "FMT_REPORT=%TEMP%\dotisle-gofmt-%RANDOM%.txt"
echo === gofmt check ===
gofmt -l . > "%FMT_REPORT%"
if errorlevel 1 exit /b 1
for %%F in ("%FMT_REPORT%") do if %%~zF GTR 0 (
  type "%FMT_REPORT%"
  del "%FMT_REPORT%"
  exit /b 1
)
del "%FMT_REPORT%"
set "CENTER_MOD=%TEMP%\DotIsle-MyGo-%RANDOM%-%RANDOM%\center.mod"
echo === MyGo file-backed fonts ===
go run ./cmd/mygo-patch -out "%CENTER_MOD%"
if errorlevel 1 exit /b 1
echo === go vet ===
go vet -modfile="%CENTER_MOD%" -tags=dotisle_mygo_fontfile ./...
if errorlevel 1 exit /b 1
echo === go build ===
go build -modfile="%CENTER_MOD%" -tags=dotisle_mygo_fontfile -trimpath -ldflags "-s -w -H=windowsgui" -o LyricIsland.Center.exe .
if errorlevel 1 exit /b 1
echo === go test all packages ===
go test -modfile="%CENTER_MOD%" -tags=dotisle_mygo_fontfile -count=1 -timeout=90s ./...
if errorlevel 1 exit /b 1
echo BUILD_AND_TESTS=PASS
exit /b 0
