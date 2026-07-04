import ApiClient from './client.js';
import * as authMethods from './auth.js';
import * as appMethods from './apps.js';
import * as dockerMethods from './docker.js';
import * as databaseMethods from './databases.js';
import * as serverMethods from './servers.js';
import * as wordpressMethods from './wordpress.js';
import * as systemMethods from './system.js';
import * as securityMethods from './security.js';
import * as fileMethods from './files.js';
import * as dnsMethods from './dns.js';
import * as cloudflareMethods from './cloudflare.js';
import * as pluginMethods from './plugins.js';
import * as deploymentJobMethods from './deploymentJobs.js';
import * as pairingMethods from './pairing.js';
import * as sourceConnectionMethods from './sourceConnections.js';
import * as connectionMethods from './connections.js';
import * as aiMethods from './ai.js';
import * as tunnelMethods from './tunnels.js';
import * as secretsWebhooksMethods from './secretsWebhooks.js';
import * as containerOpsMethods from './containerOps.js';
import * as queueBusMethods from './queueBus.js';
import * as notificationMethods from './notifications.js';
import * as telemetryMethods from './telemetry.js';
import * as jobMethods from './jobs.js';
import * as backupProtectionMethods from './backupProtection.js';
import * as containerStatusMethods from './containerStatus.js';
import * as buildpackMethods from './buildpacks.js';
import * as snapshotMethods from './snapshots.js';
import * as projectMethods from './projects.js';
import * as sharedResourceMethods from './sharedResources.js';
import * as previewMethods from './previews.js';
import * as proxyMethods from './proxy.js';
import * as magentoMethods from './magento.js';

class ApiService extends ApiClient {
    constructor() {
        super();
        // Bind all methods from domain modules to this instance
        const modules = [
            authMethods,
            appMethods,
            dockerMethods,
            databaseMethods,
            serverMethods,
            wordpressMethods,
            systemMethods,
            securityMethods,
            fileMethods,
            dnsMethods,
            cloudflareMethods,
            pluginMethods,
            deploymentJobMethods,
            pairingMethods,
            sourceConnectionMethods,
            connectionMethods,
            aiMethods,
            tunnelMethods,
            secretsWebhooksMethods,
            containerOpsMethods,
            queueBusMethods,
            notificationMethods,
            telemetryMethods,
            jobMethods,
            backupProtectionMethods,
            containerStatusMethods,
            buildpackMethods,
            snapshotMethods,
            projectMethods,
            sharedResourceMethods,
            previewMethods,
            proxyMethods,
            magentoMethods,
        ];
        for (const mod of modules) {
            for (const [key, fn] of Object.entries(mod)) {
                if (typeof fn === 'function') {
                    this[key] = fn.bind(this);
                }
            }
        }
    }
}

export const api = new ApiService();
export default api;
