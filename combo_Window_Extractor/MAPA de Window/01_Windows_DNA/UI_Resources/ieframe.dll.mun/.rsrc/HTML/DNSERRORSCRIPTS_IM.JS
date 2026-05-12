
var timeoutID;
var checkConnection = function () {
if (window.navigator.onLine) {
document.getElementById("titleHeader").textContent = L_THIS_PAGE_CANT_BE_DISPLAYED_TEXT;
document.getElementById("notConnectedTasks").style.display = "none";
document.getElementById("cantDisplayTasks").style.display = "";
addURL();
} else {
document.getElementById("titleHeader").textContent = L_YOU_ARE_NOT_CONNECTED_TEXT;
document.getElementById("notConnectedTasks").style.display = "";
document.getElementById("cantDisplayTasks").style.display = "none";
}
};
function diagnoseConnectionAndRefresh() {
var button = document.getElementById("resetButton");
button.disabled = "disabled";
var spinnerHolder = document.getElementById("spinnerHolder");
spinnerHolder.style.display = "";
document.body.removeEventListener("offline", reportConnectionEvent, false);
try {
window.external.msDiagnoseConnectionUILess();
} catch (ex) {
}
timeoutID = setTimeout(function () {
timeoutID = null;
clickRefresh();
}, 30000);
};
function getInfo() {
checkConnection();
document.body.addEventListener("online", reportConnectionEvent, false);
document.body.addEventListener("offline", reportConnectionEvent, false);
};
function reportConnectionEvent(e) {
if ("online" === e.type) {
document.body.removeEventListener("online", reportConnectionEvent, false);
if (timeoutID) {
clearTimeout(timeoutID);
timeoutID = null;
}
setTimeout("clickRefresh()", 5000);
} else {
document.body.removeEventListener("offline", reportConnectionEvent, false);
checkConnection();
}
};
function clickRefresh() {
var location = window.location.href;
var poundIndex = location.indexOf('#');
if (poundIndex != -1 && poundIndex + 1 < location.length && isExternalUrlSafeForNavigation(location.substring(poundIndex + 1))) {
window.location.replace(location.substring(poundIndex + 1));
}
};
function isExternalUrlSafeForNavigation(urlStr) {
var regEx = new RegExp("^(http(s?)|ftp|file)://", "i");
return regEx.exec(urlStr);
};
function addURL() {
var DocURL = document.location.href;
var beginIndex = DocURL.indexOf('#') + 1;
var protocolEndIndex = DocURL.indexOf("://", beginIndex);
var endIndex = DocURL.indexOf("/", protocolEndIndex + 3);
var urlResult = DocURL.substring(beginIndex, endIndex);
document.getElementById("webpage").innerText = urlResult + " ";
};
function addDomainName()
{
var domainNamePlaceholder = document.getElementById("DomainName");
domainNamePlaceholder.innerText = findValue("DomainName=") + " ";
}
function addProxyDetail()
{
var proxyDetailPlaceholder = document.getElementById("ProxyDetail");
proxyDetailPlaceholder.innerText = findValue("ProxyDetail=");
}
function findValue(key)
{
var value = '';
DocQuery = document.location.search;
BeginString = DocQuery.indexOf(key);
if (BeginString > 0)
{
BeginString += key.length;
EndString = Math.max(0, Math.min(DocQuery.indexOf("&", BeginString), DocQuery.indexOf("#", BeginString)));
if (EndString > 0)
{
value = DocQuery.substring(BeginString, EndString);
}
else
{
value = DocQuery.substring(BeginString);
}
}
return value;
}
function launchInternetOptions()
{
window.external.msLaunchInternetOptions();
}
