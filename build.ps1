$env:CMAKE = "C:\Program Files\CMake\bin\cmake.exe"
$env:Path = "C:\Program Files\CMake\bin;C:\Program Files\LLVM\bin;" + $env:Path
Set-Location "D:\CProjs\flov"
& "C:\Users\refla\.cargo\bin\cargo.exe" build --release
