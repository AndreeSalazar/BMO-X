
function wrapRegionalShortDate(dateIn)
{
var newDate;
try
{
newDate = regionalShortDate(dateIn);
}
catch(e)
{
newDate = dateIn;
}
return newDate;
}
function regionalizeDates() {
var fcDate = _fileCreatedDate.innerText;
if (fcDate!= "undefined" && fcDate != "")
_fileCreatedDate.innerText = wrapRegionalShortDate(fcDate);
var fmDate = _fileModifiedDate.innerText;
if (fmDate != "undefined" && fmDate != "")
_fileModifiedDate.innerText = wrapRegionalShortDate(fmDate);
}
function btnCertsClick()
{
window.dialogArguments.showSecurityInfo();
}
function BodyLoad()
{
var arrAll = document.all;
if(document.dir == "rtl")
{
divButtons.style.pixelLeft = 15;
}
else
{
divButtons.style.pixelLeft = document.body.clientWidth - (btnCerts.offsetWidth + 15);
}
regionalizeDates();
for (i=0; i < arrAll.length; i++)
{
var elm = arrAll[i];
if (elm.id.substring(0,1) == "_")
{
if (elm.innerText== "undefined" || elm.innerText == "")
{
elm.innerText = L_NotAvailable_Text;
if (elm.id == "_fileSize")
spaBytes.innerText = "";
else if (elm.id == "_connInfo" )
{
var secInfo = window.dialogArguments.secureConnectionInfo;
if (secInfo == "")
elm.innerText = L_NotEncrypted_Text;
else
elm.innerText = secInfo ;
}
}
}
}
}
