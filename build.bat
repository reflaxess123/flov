@echo off
set LIBCLANG_PATH=C:\Program Files\LLVM\bin
set PATH=%PATH%;C:\Program Files\CMake\bin;C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v13.0\bin
set CUDA_PATH=C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v13.0
%USERPROFILE%\.cargo\bin\cargo build --release
