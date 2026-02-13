; Shard Daemon NSIS Installer Script
; Builds Windows .exe installer with auto-update support

!include "MUI2.nsh"
!include "FileFunc.nsh"
!include "WinVer.nsh"

; General
Name "Shard"
OutFile "shard-installer.exe"
InstallDir "$PROGRAMFILES64\Shard"
InstallDirRegKey HKLM "Software\Shard" "InstallDir"
RequestExecutionLevel admin

; Version
!define VERSION "0.4.0"
!define PUBLISHER "Shard Network"
!define URL "https://shard.network"

; MUI Settings
!define MUI_ABORTWARNING
!define MUI_ICON "${NSISDIR}\Contrib\Graphics\Icons\modern-install.ico"
!define MUI_UNICON "${NSISDIR}\Contrib\Graphics\Icons\modern-uninstall.ico"

; Pages
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "LICENSE.txt"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

; Language
!insertmacro MUI_LANGUAGE "English"

; Installer Sections
Section "Shard" SecMain
    SetOutPath "$INSTDIR"
    
    ; Copy main executable
    File "target\release\shard-daemon.exe"
    
    ; Copy Python runtime if bundled
    File /r "python\*.*"
    
    ; Create data directory
    CreateDirectory "$APPDATA\shard"
    
    ; Create Start Menu shortcuts
    CreateDirectory "$SMPROGRAMS\Shard"
    CreateShortcut "$SMPROGRAMS\Shard\Shard.lnk" "$INSTDIR\shard-daemon.exe" "--contribute" "$INSTDIR\shard-daemon.exe" 0
    CreateShortcut "$SMPROGRAMS\Shard\Uninstall.lnk" "$INSTDIR\Uninstall.exe"
    
    ; Create Desktop shortcut
    CreateShortcut "$DESKTOP\Shard.lnk" "$INSTDIR\shard-daemon.exe" "--contribute" "$INSTDIR\shard-daemon.exe" 0
    
    ; Write registry keys
    WriteRegStr HKLM "Software\Shard" "InstallDir" "$INSTDIR"
    WriteRegStr HKLM "Software\Shard" "Version" "${VERSION}"
    
    ; Write uninstall info
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Shard" "DisplayName" "Shard"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Shard" "UninstallString" "$INSTDIR\Uninstall.exe"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Shard" "InstallLocation" "$INSTDIR"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Shard" "DisplayVersion" "${VERSION}"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Shard" "Publisher" "${PUBLISHER}"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Shard" "URLInfoAbout" "${URL}"
    WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Shard" "NoModify" 1
    WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Shard" "NoRepair" 1
    
    ; Get installed size
    ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
    IntFmt $0 "0x%08X" $0
    WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Shard" "EstimatedSize" "$0"
    
    ; Write uninstaller
    WriteUninstaller "$INSTDIR\Uninstall.exe"
SectionEnd

; Uninstaller Section
Section "Uninstall"
    ; Kill running processes
    nsExec::ExecToLog 'taskkill /F /IM shard-daemon.exe'
    
    ; Remove files
    Delete "$INSTDIR\shard-daemon.exe"
    Delete "$INSTDIR\Uninstall.exe"
    RMDir /r "$INSTDIR"
    
    ; Remove shortcuts
    Delete "$SMPROGRAMS\Shard\*.lnk"
    RMDir "$SMPROGRAMS\Shard"
    Delete "$DESKTOP\Shard.lnk"
    
    ; Remove registry keys
    DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Shard"
    DeleteRegKey HKLM "Software\Shard"
    
    ; Optionally keep data directory
    ; RMDir /r "$APPDATA\shard"
SectionEnd
