
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
function BodyLoad()
{
var arrAll = document.all;
regionalizeDates();
for (i=0; i < arrAll.length; i++)
{
var elm = arrAll[i];
if (elm.id.substring(0,1) == "_")
{
if (elm.innerText== "undefined" || elm.innerText == "" || elm.innerText == "-1")
{
elm.innerText = L_NotAvailable_Text;
switch (elm.id)
{
case "_fileSize":
spaBytes.innerText = "";
break;
case "_width":
case "_height":
spaX.innerText = "";
spaPixels.innerText = "";
_width.innerText = "";
_height.innerText = L_NotAvailable_Text;
break;
}
}
}
}
}
