
function isExternalUrlSafeForNavigation(urlStr)
{
var regEx = new RegExp("^(http(s?)|ftp|file)://", "i");
return regEx.exec(urlStr);
}
function clickRefresh()
{
var location = window.location.href;
var poundIndex = location.indexOf('#');
if (poundIndex != -1 && poundIndex+1 < location.length && isExternalUrlSafeForNavigation(location.substring(poundIndex+1)))
{
window.location.replace(location.substring(poundIndex+1));
}
}
function navCancelInit()
{
var location = window.location.href;
var poundIndex = location.indexOf('#');
if (poundIndex != -1 && poundIndex+1 < location.length && isExternalUrlSafeForNavigation(location.substring(poundIndex+1)))
{
var bElement = document.createElement("A");
bElement.innerText = L_REFRESH_TEXT;
bElement.href = 'javascript:clickRefresh()';
navCancelContainer.appendChild(bElement);
}
else
{
var textNode = document.createTextNode(L_RELOAD_TEXT);
navCancelContainer.appendChild(textNode);
}
}
function getDisplayValue(elem)
{
var objStyle;
if (window.getComputedStyle)
{
objStyle = window.getComputedStyle(elem);
}
else
{
objStyle = elem.currentStyle;
}
return objStyle.display;
}
function expandCollapse(elem, changeImage)
{
if (document.getElementById)
{
ecBlock = document.getElementById(elem);
if (ecBlock != undefined && ecBlock != null)
{
if (changeImage)
{
elemImage = document.getElementById(elem + "Image");
}
if (!changeImage || (elemImage != undefined && elemImage != null))
{
var displayValue = getDisplayValue(ecBlock);
if (displayValue == "none" || displayValue == null || displayValue == "")
{
ecBlock.style.display = "block";
if (changeImage)
{
elemImage.src = "up.png";
}
}
else if (displayValue == "block")
{
ecBlock.style.display = "none";
if (changeImage)
{
elemImage.src = "down.png";
}
}
else
{
ecBlock.style.display = "block";
if (changeImage)
{
elemImage.src = "up.png";
}
}
}
}
}
}
function initHomepage()
{
DocURL=document.location.href;
var poundIndex = DocURL.indexOf('#');
if (poundIndex != -1 && poundIndex+1 < location.length && isExternalUrlSafeForNavigation(location.substring(poundIndex+1)))
{
protocolIndex=DocURL.indexOf("://", 4);
serverIndex=DocURL.indexOf("/", protocolIndex + 3);
BeginURL=DocURL.indexOf("#",1) + 1;
urlresult=DocURL.substring(BeginURL, serverIndex);
if (protocolIndex - BeginURL > 7)
urlresult="";
displayresult=DocURL.substring(protocolIndex + 3, serverIndex);
}
else
{
displayresult = "";
urlresult = "";
}
var aElement = document.createElement("A");
aElement.innerText = displayresult;
aElement.href = urlresult;
homepageContainer.appendChild(aElement);
}
function initConnectionStatus()
{
if (navigator.onLine)
{
checkConnection.innerText = L_CONNECTION_ON_TEXT;
}
else
{
checkConnection.innerText = L_CONNECTION_OFF_TEXT;
}
}
function initGoBack()
{
if (history.length < 1)
{
var textNode = document.createTextNode(L_GOBACK_TEXT);
goBackContainer.appendChild(textNode);
}
else
{
var bElement = document.createElement("A");
bElement.innerText = L_GOBACK_TEXT ;
bElement.href = "javascript:history.back();";
goBackContainer.appendChild(bElement);
}
}
function initMoreInfo(infoBlockID)
{
var bElement = document.createElement("A");
bElement.innerText = L_MOREINFO_TEXT;
bElement.href = "javascript:expandCollapse(\'infoBlockID\', true);";
moreInfoContainer.appendChild(bElement);
}
function initOfflineUser(offlineUserID)
{
var bElement = document.createElement("A");
bElement.innerText = L_OFFLINE_USERS_TEXT;
bElement.href = "javascript:expandCollapse('offlineUserID', true);";
offlineUserContainer.appendChild(bElement);
}
function initUnframeContent()
{
var location = window.location.href;
var poundIndex = location.indexOf('#');
if (poundIndex != -1 && poundIndex+1 < location.length && isExternalUrlSafeForNavigation(location.substring(poundIndex+1)))
{
document.getElementById("whatToDoIntro").style.display="";
document.getElementById("whatToDoBody").style.display="";
}
}
function removeNoScriptElements() {
var noScriptElements = document.getElementsByTagName("noscript");
for (var i = noScriptElements.length - 1; i >= 0; i--)
{
var bElement = noScriptElements[i];
if (bElement !== undefined && bElement !== null)
{
bElement.parentNode.removeChild(bElement);
}
}
}
function makeNewWindow()
{
var location = window.location.href;
var poundIndex = location.indexOf('#');
if (poundIndex != -1 && poundIndex+1 < location.length && isExternalUrlSafeForNavigation(location.substring(poundIndex+1)))
{
window.open(location.substring(poundIndex+1));
}
}
function setTabInfo(tabInfoBlockID)
{
var bPrevElement = document.getElementById("tabInfoTextID");
var bPrevImage = document.getElementById("tabInfoBlockIDImage");
if (bPrevElement != null)
{
tabInfoContainer.removeChild(bPrevElement);
}
if (bPrevImage != null)
{
tabImageContainer.removeChild(bPrevImage);
}
var bElement = document.createElement("A");
var bImageElement = document.createElement("IMG");
var ecBlock = document.getElementById(tabInfoBlockID);
if ((ecBlock != undefined && ecBlock != null) &&
(getDisplayValue(ecBlock) == "none" || getDisplayValue(ecBlock) == null || getDisplayValue(ecBlock) == ""))
{
bElement.innerText = L_SHOW_HOTKEYS_TEXT;
bImageElement.alt = L_SHOW_HOTKEYS_TEXT;
bImageElement.src="down.png";
}
else
{
bElement.innerText = L_HIDE_HOTKEYS_TEXT;
bImageElement.alt = L_HIDE_HOTKEYS_TEXT;
bImageElement.src="up.png";
}
bElement.id = "tabInfoTextID";
bElement.href = "javascript:expandCollapse(\'tabInfoBlockID\', false); setTabInfo('tabInfoBlockID');";
bImageElement.id="tabInfoBlockIDImage";
bImageElement.border="0";
bImageElement.className="actionIcon";
tabInfoContainer.appendChild(bElement);
tabImageContainer.appendChild(bImageElement);
}
function launchInternetOptions()
{
window.external.msLaunchInternetOptions();
}
function diagnoseConnection()
{
window.external.DiagnoseConnection();
}
function diagnoseConnectionAndRefresh()
{
window.external.DiagnoseConnection();
if (navigator.onLine)
{
clickRefresh();
}
}
function getInfo()
{
checkConnection();
if (document.addEventListener)
{
addEventListener("offline", reportConnectionEvent, false);
}
else
{
attachEvent("onoffline", reportConnectionEvent);
}
document.body.ononline = reportConnectionEvent;
document.body.onoffline = reportConnectionEvent;
}
function checkConnection()
{
var newHeading = document.getElementById("mainTitle");
var notConnectedTasks = document.getElementById("notConnectedTasks");
var cantDisplayTasks = document.getElementById("cantDisplayTasks");
if (navigator.onLine)
{
document.title = L_INTERNET_CONNECTED_TEXT;
newHeading.textContent = L_INTERNET_CONNECTED_TEXT;
addURL();
getErrorStatus();
notConnectedTasks.style.display = "none";
cantDisplayTasks.style.display = "";
}
else
{
document.title = L_INTERNET_NOT_CONNECTED_TEXT;
newHeading.textContent = L_INTERNET_NOT_CONNECTED_TEXT;
notConnectedTasks.style.display = "";
cantDisplayTasks.style.display = "none";
}
}
function reportConnectionEvent(e)
{
if (!e) e = window.event;
if ('online' == e.type)
{
setTimeout ( "clickRefresh()", 1000 );
}
else if ('offline' == e.type)
{
checkConnection();
}
else
{
checkConnection();
}
}
function addURL()
{
var urlResult = "";
var DocURL = document.location.href;
var urlPlaceholder = document.getElementById("webpage");
var beginIndex = DocURL.indexOf('#') + 1;
if (DocURL.indexOf("file://", beginIndex) == -1)
{
var protocolEndIndex = DocURL.indexOf("://", beginIndex);
var endIndex=DocURL.indexOf("/", protocolEndIndex + 3);
urlResult = DocURL.substring(beginIndex, endIndex);
urlPlaceholder.innerText = urlResult + " ";
var cantDisplayTasks = document.getElementById("cantDisplayTasks");
if (cantDisplayTasks) {
var searchTask = document.getElementById("task1-2");
var searchString = searchTask.innerText;
var bingLink = document.createElement("a");
bingLink.href = "https://www.bing.com/search?q=" + encodeURIComponent(urlResult) + "&PC=IENEPB";
bingLink.innerHTML = searchString;
searchTask.innerHTML = "";
searchTask.appendChild(bingLink);
}
}
}
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
DocQuery = document.URL;
BeginString = DocQuery.indexOf(key);
if (BeginString > 0)
{
BeginString += key.length;
var ampLocation = DocQuery.indexOf("&", BeginString);
var poundLocation = DocQuery.indexOf("#", BeginString);
var EndString = 0;
if (ampLocation > 0 && poundLocation > 0)
{
EndString = Math.min(ampLocation, poundLocation);
}
else if (ampLocation > 0)
{
EndString = ampLocation;
}
else if (poundLocation > 0)
{
EndString = poundLocation;
}
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
function isHTTPS(cantDisplayTasks)
{
var DocURL = document.location.href;
var poundIndex = DocURL.indexOf('#');
var protocolIndex = DocURL.indexOf("https://", poundIndex);
if (protocolIndex>poundIndex)
{
var bElement = document.createElement("li");
bElement.textContent = L_TLS_SSL_TEXT;
cantDisplayTasks.appendChild(bElement);
}
}
function getErrorStatus() {
var errorStatus = findValue("ErrorStatus=");
if (errorStatus)
{
var errorCode = "";
var subError = document.getElementById("subError");
var errorCodes = {
"0x800C0008": { name: "INET_E_DOWNLOAD_FAILURE", description: L_DOWNLOAD_FAILURE_TEXT },
"0x800C0014": { name: "INET_E_REDIRECT_FAILED", description: L_REDIRECT_FAILED_TEXT },
"0x800C000B": { name: "INET_E_CONNECTION_TIMEOUT", description: L_CONNECTION_TIMEOUT_TEXT },
"0x800C0007": { name: "INET_E_DATA_NOT_AVAILABLE", description: L_DATA_NOT_AVAILABLE_TEXT },
"0x800C0004": { name: "INET_E_CANNOT_CONNECT", description: null },
"0x800C000E": { name: "INET_E_SECURITY_PROBLEM", description: null },
"0x800C0006": { name: "INET_E_OBJECT_NOT_FOUND", description: null },
"0x800C0002": { name: "INET_E_INVALID_URL", description: null }
};
var ERROR_SUCCESS = 0;
var WSAECONNRESET = 10054;
var ERROR_TIMEOUT = 1460;
var DNS_ERROR_RCODE_NAME_ERROR = 9003;
var INET_E_RESOURCE_NOT_FOUND = "0x800C0005";
if (errorStatus === INET_E_RESOURCE_NOT_FOUND)
{
errorCode = "INET_E_RESOURCE_NOT_FOUND";
var dnsString = findValue("DNSError=");
if (dnsString)
{
var dnsStatus = Number(dnsString);
if (dnsStatus === WSAECONNRESET)
{
errorString = L_RESOURCE_NOT_FOUND_WSAECONNRESET_TEXT;
}
else if (dnsStatus === ERROR_TIMEOUT)
{
errorString = L_RESOURCE_NOT_FOUND_ERROR_TIMEOUT_TEXT;
}
else if (dnsStatus === ERROR_SUCCESS)
{
errorString = L_RESOURCE_NOT_FOUND_ERROR_SUCCESS_TEXT;
}
else if (dnsStatus === DNS_ERROR_RCODE_NAME_ERROR)
{
errorString = L_RESOURCE_NOT_FOUND_RCODE_NAME_ERROR_TEXT;
}
else
{
errorString = L_RESOURCE_NOT_FOUND_ERROR_OTHER_TEXT;
}
}
else
{
errorString = L_RESOURCE_NOT_FOUND_TEXT;
}
document.getElementById("ErrorCode").innerHTML = L_ERROR_CODE_TEXT + errorCode;
subError.innerHTML = errorString + "<br>";
}
else if (errorStatus in errorCodes)
{
var error = errorCodes[errorStatus];
document.getElementById("ErrorCode").innerHTML = L_ERROR_CODE_TEXT + error.name;
if (error.description)
{
subError.innerHTML = error.description + "<br>";
}
}
}
}
function checkTLSError()
{
var protocolSettings = Number(findValue("SecureProtocol="));
var defaultSettings = 2048;
if (protocolSettings !== defaultSettings)
{
document.getElementById("subError").innerHTML += "<br><br>" + L_NONDEFAULT_SETTINGS_TEXT;
document.getElementById("internetOptions").className = document.getElementById("internetOptions").className.replace(/(?:^|\s)hidden(?!\S)/g, '');
}
}
