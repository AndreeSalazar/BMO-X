
function HandleError(message, url, line)
{
var str = L_Dialog_ErrorMessage + "\n\n"
+ L_ErrorNumber_Text + line + "\n"
+ message;
alert (str);
window.close();
return true;
}
function BodyOnload ()
{
Printer.showPageSetupDialog();
window.close();
}
window.onerror = HandleError;
