@Echo off
pushd %~d1%~p1
call pdfcrop.exe %~nx1 --verbose
popd
pause