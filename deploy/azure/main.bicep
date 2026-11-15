// The Husker online server on Azure Functions, Flex Consumption plan.
//
// The function app runs `husker serve` as a custom handler. It reaches the
// sync file with a SAS that this template creates from a stored access policy.
// The SAS goes straight into the app settings and is never an output.

@description('Name of the function app. The server address becomes https://<app>.azurewebsites.net.')
param app string

@description('Region of the function app. Use the region of the sync file.')
param location string = resourceGroup().location

@description('The existing storage account that holds the sync file.')
param syncAccount string

@description('The container of the sync file.')
param syncContainer string = 'husker'

@description('The blob name of the sync file.')
param syncBlob string = 'tasks.json'

@description('The stored access policy on the container that the server SAS refers to. Deleting the policy cuts the server off.')
param syncPolicy string = 'husker-online'

@description('Name of the new storage account for the function host and the code packages.')
param hostAccount string

@description('Public keys from "husker token --public-key", separated by commas.')
param onlineKeys string

@description('Token IDs that the server refuses, separated by commas.')
param revoked string = ''

@description('IANA time zone that decides "today" for MCP calls.')
param timeZone string = 'Europe/Copenhagen'

@description('True removes write access from every request. Use it for the first deployment.')
param readOnly bool = true

@description('The most instances the app may scale to. One bounds the cost and keeps the file cache in one place.')
param maxInstances int = 1

var origin = 'https://${app}.azurewebsites.net'

resource syncStorage 'Microsoft.Storage/storageAccounts@2023-05-01' existing = {
  name: syncAccount
}

// The host storage is separate, so the function host never holds a key of the account with the tasks.
resource hostStorage 'Microsoft.Storage/storageAccounts@2023-05-01' = {
  name: hostAccount
  location: location
  kind: 'StorageV2'
  sku: {
    name: 'Standard_LRS'
  }
  properties: {
    minimumTlsVersion: 'TLS1_2'
    supportsHttpsTrafficOnly: true
    allowBlobPublicAccess: false
  }
}

resource hostBlobs 'Microsoft.Storage/storageAccounts/blobServices@2023-05-01' = {
  parent: hostStorage
  name: 'default'
}

// `app-package` is where the platform keeps the deployed code. `packages` receives uploads.
resource containers 'Microsoft.Storage/storageAccounts/blobServices/containers@2023-05-01' = [
  for name in ['app-package', 'packages']: {
    parent: hostBlobs
    name: name
  }
]

var hostConnection = 'DefaultEndpointsProtocol=https;AccountName=${hostStorage.name};AccountKey=${hostStorage.listKeys().keys[0].value};EndpointSuffix=${environment().suffixes.storage}'

// A SAS for the one blob. Its permissions and expiry come from the stored access policy.
var syncSas = syncStorage.listServiceSas('2023-05-01', {
  canonicalizedResource: '/blob/${syncAccount}/${syncContainer}/${syncBlob}'
  signedResource: 'b'
  signedIdentifier: syncPolicy
  signedProtocol: 'https'
}).serviceSasToken

resource plan 'Microsoft.Web/serverfarms@2024-04-01' = {
  name: '${app}-plan'
  location: location
  kind: 'functionapp'
  sku: {
    tier: 'FlexConsumption'
    name: 'FC1'
  }
  properties: {
    reserved: true
  }
}

resource site 'Microsoft.Web/sites@2024-04-01' = {
  name: app
  location: location
  kind: 'functionapp,linux'
  dependsOn: [
    containers
  ]
  properties: {
    serverFarmId: plan.id
    httpsOnly: true
    functionAppConfig: {
      runtime: {
        name: 'custom'
        version: '1.0'
      }
      scaleAndConcurrency: {
        instanceMemoryMB: 512
        maximumInstanceCount: maxInstances
        triggers: {
          http: {
            perInstanceConcurrency: 50
          }
        }
      }
      deployment: {
        storage: {
          type: 'blobContainer'
          value: '${hostStorage.properties.primaryEndpoints.blob}app-package'
          authentication: {
            type: 'StorageAccountConnectionString'
            storageAccountConnectionStringName: 'AzureWebJobsStorage'
          }
        }
      }
    }
    siteConfig: {
      minTlsVersion: '1.2'
      appSettings: [
        {
          name: 'AzureWebJobsStorage'
          value: hostConnection
        }
        {
          name: 'AzureWebJobsDisableHomepage'
          value: 'true'
        }
        {
          name: 'HUSKER_SYNC_URL'
          value: '${syncStorage.properties.primaryEndpoints.blob}${syncContainer}/${syncBlob}?${syncSas}'
        }
        {
          name: 'HUSKER_ONLINE_ORIGIN'
          value: origin
        }
        {
          name: 'HUSKER_ONLINE_KEYS'
          value: onlineKeys
        }
        {
          name: 'HUSKER_ONLINE_REVOKED'
          value: revoked
        }
        {
          name: 'HUSKER_ONLINE_TZ'
          value: timeZone
        }
        {
          name: 'HUSKER_ONLINE_READ_ONLY'
          value: readOnly ? 'true' : 'false'
        }
      ]
    }
  }
}

output url string = origin
output hostAccount string = hostStorage.name
