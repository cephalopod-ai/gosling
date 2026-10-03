import type {
  AuthenticationExtensionSetRequest_unstable,
  AuthenticationTarget,
} from '@repo-makeover/gosling-sdk';
import { getAcpClient } from './acpConnection';

export async function acpReadAuthentication(target: AuthenticationTarget) {
  const client = await getAcpClient();
  return client.gosling.authenticationRead_unstable({ target });
}

export async function acpSetProviderAuthentication(
  target: AuthenticationTarget,
  profileId: string | null
) {
  const client = await getAcpClient();
  return client.gosling.authenticationProviderSet_unstable({ target, profileId });
}

export async function acpSetExtensionAuthentication(
  request: AuthenticationExtensionSetRequest_unstable
) {
  const client = await getAcpClient();
  return client.gosling.authenticationExtensionSet_unstable(request);
}
