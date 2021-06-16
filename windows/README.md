# Windows application metadata

By default Windows has very strict backwards compatibility requirements to ensure old applications don't break on modern Windows.
However, sometimes it's desirable to make breaking changes.

[Application manifests](https://docs.microsoft.com/en-us/windows/win32/sbscs/application-manifests) provide a way to do both.
Older application without a manifest will continue to have the old, backwards compatible, behaviour.
Newer applications can use the manifest to opt in to new features.

A feature of particular use to ripgrep is long path support.

## Windows long paths

Newer versions of Windows 10 allow applications to use longer paths without making any changes to the way they use the Windows API. Unfortunately enabling this setting requires both the application and the user to opt in to this behaviour.

## Making ripgrep `longPathAware`

The `longPathAware` setting is enabled in the [manifest](manifest.xml). Building ripgrep with this manifest requires the following black magic (and an MSVC compatible linker):

```
cargo rustc --release -- -C link-arg="/MANIFEST:EMBED" -C link-arg="/MANIFESTINPUT:windows/manifest.xml"
```

However, even then this setting will be ignored unless the user has allowed long path support.

## Enabling long paths on your OS

For this to work you will need to be using Windows 10 version 1607 or later and have admin rights.

It requires setting a registry entry, which can be done in one of the following ways:

### Using a `.reg` file

Simply download and then open the [LongPathsEnabled.reg](LongPathsEnabled.reg) file.

### Using the Group Policy Editor

Run `gpedit.msc`. Navigate to and enable:

Computer Configuration > Administrative Templates > System > Filesystem > Enable Win32 long paths

### Using an administrator powershell

Run powershell as admin and use the following command:

```ps
New-ItemProperty -Path "HKLM:\SYSTEM\CurrentControlSet\Control\FileSystem" `
-Name "LongPathsEnabled" -Value 1 -PropertyType DWORD -Force
```
