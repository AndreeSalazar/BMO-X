
function CertError()
{
error = '0';
DocQuery=document.location.search;
BeginError = DocQuery.indexOf("SSLError=");
if (BeginError > 0)
{
BeginError += 9;
EndError = DocQuery.indexOf("&", BeginError);
if (EndError > 0)
{
error = DocQuery.substring(BeginError,EndError);
}
else
{
error = DocQuery.substring(BeginError);
}
}
return error;
}
function PreventIgnoreCertErrors()
{
Policy = '0';
DocQuery=document.location.search;
BeginPolicy = DocQuery.indexOf("PreventIgnoreCertErrors=")+24;
if (BeginPolicy > 0)
{
EndPolicy = DocQuery.indexOf("&", BeginPolicy);
if (EndPolicy > 0)
{
Policy = DocQuery.substring(BeginPolicy,EndPolicy);
}
else
{
Policy = DocQuery.substring(BeginPolicy);
}
}
return Policy;
}
function closePage() {
window.close();
}
function BodyLoad()
{
var iError = CertError();
var iPolicy = PreventIgnoreCertErrors();
var sRealPageUrl = RealPageURL();
var iCertUnknownCA = 16777216;
var iCertExpired = 67108864;
var iCertCNMismatch = 33554432;
var iCertRevoked = 12170;
var iCertWeakSignature = 2097152;
var certReasons = [];
var errorCodes = [];
if (iError == iCertRevoked)
{
certReasons.push(L_CertRevoked_TEXT);
errorCodes.push("ERROR_INTERNET_SEC_CERT_REVOKED");
if (document.getElementById("override")) {
document.getElementById("override").style.display = "none";
}
}
else
{
if (iError & iCertUnknownCA)
{
certReasons.push(L_CertUnknownCA_TEXT);
errorCodes.push("DLG_FLAGS_INVALID_CA");
}
if (iError & iCertExpired)
{
certReasons.push(L_CertExpired_TEXT);
errorCodes.push("DLG_FLAGS_SEC_CERT_DATE_INVALID");
}
if (iError & iCertCNMismatch)
{
certReasons.push(L_CertCNMismatch_TEXT);
errorCodes.push("DLG_FLAGS_SEC_CERT_CN_INVALID");
}
if (iError == 0)
{
certReasons.push(L_CertSigFailed_TEXT);
errorCodes.push("0");
}
if (iError & iCertWeakSignature)
{
if (typeof L_CertWeakSignature_TEXT != 'undefined')
{
certReasons.push(L_CertWeakSignature_TEXT);
errorCodes.push("DLG_FLAGS_WEAK_SIGNATURE");
}
else
{
certReasons.push(L_CertSigFailed_TEXT);
errorCodes.push("0");
}
}
if (document.getElementById("override")) {
if (iPolicy == 1 || iError == 0) {
document.getElementById("override").style.display = "none";
}
else {
document.getElementById("overridelink").href = sRealPageUrl;
}
}
}
if (certReasons.length > 0) {
document.getElementById("certReason").innerHTML = certReasons.join("<br>");
}
if (errorCodes.length > 0) {
document.getElementById("ErrorCode").innerHTML = L_ERROR_CODE_TEXT + errorCodes.join("<br>");
}
removeNoScriptElements();
}
function RealPageURL()
{
urlresult = '';
DocURL=document.location.href;
BeginURL=DocURL.indexOf("#",1) + 1;
urlresult=DocURL.substring(BeginURL);
if (!isExternalUrlSafeForNavigation(urlresult))
{
urlresult = window.document.location.toString();
}
return urlresult;
}
