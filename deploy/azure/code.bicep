// Deploys one code package to the function app.
//
// The package is a zip in the `packages` container of the host storage
// account. The template creates a short-lived read SAS for it, so no secret
// passes through a terminal.

@description('Name of the function app.')
param app string

@description('The storage account of the function host.')
param hostAccount string

@description('The blob name of the package in the packages container.')
param packageBlob string

@description('The deployment time. The package SAS expires two hours later.')
param now string = utcNow()

resource hostStorage 'Microsoft.Storage/storageAccounts@2023-05-01' existing = {
  name: hostAccount
}

resource site 'Microsoft.Web/sites@2024-04-01' existing = {
  name: app
}

var packageSas = hostStorage.listServiceSas('2023-05-01', {
  canonicalizedResource: '/blob/${hostAccount}/packages/${packageBlob}'
  signedResource: 'b'
  signedPermission: 'r'
  signedProtocol: 'https'
  signedExpiry: dateTimeAdd(now, 'PT2H')
}).serviceSasToken

resource deployment 'Microsoft.Web/sites/extensions@2024-04-01' = {
  parent: site
  name: 'onedeploy'
  properties: {
    packageUri: '${hostStorage.properties.primaryEndpoints.blob}packages/${packageBlob}?${packageSas}'
    remoteBuild: false
  }
}
