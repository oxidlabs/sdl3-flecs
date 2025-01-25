@echo off

rem Process vertex shaders
for %%f in (*.vert.hlsl) do (
    if exist "%%f" (
        shadercross "%%f" -o "../Compiled/SPIRV/%%~nf.spv"
        shadercross "%%f" -o "../Compiled/MSL/%%~nf.msl"
        shadercross "%%f" -o "../Compiled/DXIL/%%~nf.dxil"
    )
)

rem Process fragment shaders
for %%f in (*.frag.hlsl) do (
    if exist "%%f" (
        shadercross "%%f" -o "../Compiled/SPIRV/%%~nf.spv"
        shadercross "%%f" -o "../Compiled/MSL/%%~nf.msl"
        shadercross "%%f" -o "../Compiled/DXIL/%%~nf.dxil"
    )
)

rem Process compute shaders
for %%f in (*.comp.hlsl) do (
    if exist "%%f" (
        shadercross "%%f" -o "../Compiled/SPIRV/%%~nf.spv"
        shadercross "%%f" -o "../Compiled/MSL/%%~nf.msl"
        shadercross "%%f" -o "../Compiled/DXIL/%%~nf.dxil"
    )
)