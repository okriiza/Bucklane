import { defineStore } from "pinia";
import { computed, ref } from "vue";

import {
  commands,
  toAppError,
  unwrap,
  type AppError,
  type ConnectionConfigInput,
  type ConnectionDto,
  type ProviderProfile,
  type ProviderType,
  type TestConnectionInput,
} from "@/services/tauri";

/**
 * Connection metadata only. Secrets are never kept here; they live in the
 * form's local state until submitted (TSD §10.2).
 */
export const useConnectionStore = defineStore("connections", () => {
  const connections = ref<ConnectionDto[]>([]);
  const profiles = ref<ProviderProfile[]>([]);
  const loading = ref(false);
  const loadError = ref<AppError | null>(null);

  /** Drives the shared add/edit dialog: `null` = closed, `"new"` = create. */
  const formTarget = ref<string | null>(null);

  const profileByProvider = computed(
    () => new Map(profiles.value.map((p) => [p.provider, p] as const)),
  );

  function profileFor(provider: ProviderType) {
    return profileByProvider.value.get(provider);
  }

  function byId(id: string) {
    return connections.value.find((c) => c.id === id);
  }

  async function load() {
    loading.value = true;
    try {
      const [list, providerProfiles] = await Promise.all([
        unwrap(commands.listConnections()),
        profiles.value.length ? profiles.value : commands.listProviderProfiles(),
      ]);
      connections.value = list;
      profiles.value = providerProfiles;
      loadError.value = null;
    } catch (err) {
      loadError.value = toAppError(err);
    } finally {
      loading.value = false;
    }
  }

  async function create(input: ConnectionConfigInput) {
    const created = await unwrap(commands.createConnection(input));
    await load();
    return created;
  }

  async function update(id: string, input: ConnectionConfigInput) {
    const updated = await unwrap(commands.updateConnection({ id, ...input }));
    await load();
    return updated;
  }

  async function remove(id: string) {
    const result = await unwrap(commands.deleteConnection(id));
    await load();
    return result;
  }

  function test(input: TestConnectionInput) {
    return unwrap(commands.testConnection(input));
  }

  return {
    connections,
    profiles,
    loading,
    loadError,
    formTarget,
    profileFor,
    byId,
    load,
    create,
    update,
    remove,
    test,
    openCreateForm: () => (formTarget.value = "new"),
    openEditForm: (id: string) => (formTarget.value = id),
    closeForm: () => (formTarget.value = null),
  };
});
