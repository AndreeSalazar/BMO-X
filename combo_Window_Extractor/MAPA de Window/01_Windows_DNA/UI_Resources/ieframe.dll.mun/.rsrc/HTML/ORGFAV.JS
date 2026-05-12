
window.onerror = HandleError;
var g_strUrl;
var g_rgUrlsToSynch = new Array(0);
var g_fSubscriptionsEnabled;
var g_dxEllipses = 0;
var g_dxEllipsesBold = 0;
function callHelp(elm)
{
if (null != elm.helpid)
{
window.showHelp(elm.helpfile, "" + parseInt(elm.helpid), "popup");
}
else
{
if ("BODY" != elm.tagName)
{
callHelp(elm.parentElement);
}
}
}
document.onhelp = new Function("callHelp(window.event.srcElement)");
function documentMouseUp()
{
if (window.event.button == 2)
{
callHelp(window.event.srcElement);
}
}
function HandleError(message, url, line)
{
var str = L_Dialog_ErrorMessage + "\n\n"
+ L_ErrorNumber_Text + line + "\n"
+ message;
alert(str);
window.close();
return true;
}
function BodyOnKeyPress(nCode)
{
if (nCode == 27)
{
window.close();
return;
}
}
function MeasureText(strText, fBold, widthMaximum)
{
var oSpanToUse = (fBold ? spanMeasureBold : spanMeasure);
if (oSpanToUse.offsetWidth < widthMaximum)
return strText;
var dxEllipses = (fBold ? g_dxEllipsesBold : g_dxEllipses);
var iCutoff = strText.length * (widthMaximum / (oSpanToUse.offsetWidth + dxEllipses));
oSpanToUse.innerText = strText.substring(0, iCutoff - 1) + L_Ellipses_Text;
if (fBold)
{
iCutoff += 3;
}
for (cExtraCutoff = 1; (oSpanToUse.offsetWidth > widthMaximum) && (cExtraCutoff < 30); cExtraCutoff++)
{
oSpanToUse.innerText = spanMeasure.innerText.substring(0, iCutoff-cExtraCutoff) + L_Ellipses_Text;
}
return oSpanToUse.innerText;
}
function OnSelectionChange(cItems, hItem, strName, strUrl, cVisits, strDate, fAvailableOffline)
{
if (cItems == -1)
{
BodyOnKeyPress(27);
return;
}
if (cItems == 0)
{
return;
}
g_strUrl = strUrl;
if (g_dxEllipses == 0)
{
spanMeasure.innerText = L_Ellipses_Text;
g_dxEllipses = spanMeasure.offsetWidth;
spanMeasureBold.innerText = L_Ellipses_Text;
g_dxEllipsesBold = spanMeasureBold.offsetWidth;
}
spanMeasureBold.innerText = strName;
spanMeasure.innerText = strUrl;
spanMeasure.innerText = "";
}
function OnResize()
{
document.recalc(true);
}
function TdNscHeight()
{
return Math.max(100, document.body.offsetHeight - tdNsc.offsetTop - rowButtons.offsetHeight
- rowHR.offsetHeight - rowDone.offsetHeight - tableMain.border - 24);
}
function EnableButtons()
{
g_fSubscriptionsEnabled = false;
var pszInitialDir = window.dialogArguments;
if (pszInitialDir && (pszInitialDir.length > 0))
{
nsc.SetRoot(pszInitialDir);
}
tdNsc.blur();
tdNewFolder.focus();
}
function BuildSynchList()
{
window.returnValue = "";
for (strUrl in g_rgUrlsToSynch)
{
window.returnValue += strUrl + "\0";
}
}
