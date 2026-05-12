
window.onerror = HandleError
function HandleError(message, url, line)
{
var str = L_Dialog_ErrorMessage + "\n\n"
+ L_ErrorNumber_Text + line + "\n"
+ message;
alert (str);
window.close();
return true;
}
function loadBdy()
{
var objOptions = window.dialogArguments;
btnNo.onclick = new Function("btnOKClick()");
btnNo.onkeydown = new Function("SwitchFocus()");
btnYes.onclick = new Function("btnYesClick()");
btnYes.onkeydown = new Function("SwitchFocus()");
document.onkeypress = new Function("docKeypress()");
spnLine.innerText = objOptions.getAttribute("errorLine");
spnCharacter.innerText = objOptions.getAttribute("errorCharacter");
spnError.innerText = objOptions.getAttribute("errorMessage");
spnCode.innerText = objOptions.getAttribute("errorCode");
txaURL.innerText = objOptions.getAttribute("errorUrl");
if (objOptions.errorDebug)
{
divDebug.innerText = L_ContinueScript_Message;
}
btnYes.focus();
}
function SwitchFocus()
{
var HTML_KEY_ARROWLEFT = 37;
var HTML_KEY_ARROWDOWN = 40;
var iCode = event.keyCode;
var strSourceID = event.srcElement.id;
if (iCode < HTML_KEY_ARROWLEFT || iCode > HTML_KEY_ARROWDOWN)
return;
if (strSourceID == "btnYes")
{
btnNo.focus();
}
else
{
btnYes.focus();
}
}
function btnOKClick()
{
window.close();
}
function btnYesClick()
{
window.returnValue = true;
window.close();
}
function docKeypress()
{
var intKeyCode = window.event.keyCode;
if (intKeyCode == L_AffirmativeKeyCodeLowerCase_Number
|| intKeyCode == L_AffirmativeKeyCodeUpperCase_Number)
{
btnYesClick();
}
if (intKeyCode == L_NegativeKeyCodeLowerCase_Number
|| intKeyCode == L_NegativeKeyCodeUpperCase_Number)
{
btnOKClick();
}
}
