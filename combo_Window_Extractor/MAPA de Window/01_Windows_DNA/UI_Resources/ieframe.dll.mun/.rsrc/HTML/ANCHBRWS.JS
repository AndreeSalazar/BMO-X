
function BodyLoad()
{
var arrAll = document.all;
for (i=0; i < arrAll.length; i++)
{
var elm = arrAll[i];
if (elm.id.substring(0,1) == "_")
{
if (elm.innerText== "undefined" || elm.innerText == "")
{
elm.innerText = L_NotAvailable_Text;
}
}
}
}
