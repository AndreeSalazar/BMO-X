
function printSite(site)
{
siteContainer.innerText = site;
}
function BodyOnKeyPress(nCode)
{
if (nCode == 27)
{
window.returnValue = 0;
window.close();
}
}
