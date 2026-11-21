/** Domain-neutral Qoin contracts. Vertical-specific schemas live in adapters. */
export type QoinDomainId = string;

export type QoinEntityRef = {
  domain: QoinDomainId;
  entity_type: string;
  entity_id: string;
};

export type QoinDataSource = {
  provider: string;
  stream: string;
};

export type QoinObservation = {
  observation_id: string;
  entity: QoinEntityRef;
  source: QoinDataSource;
  observed_at_ms: number;
  kind: string;
  measurements: Record<string, number>;
  quality: number;
  attributes?: Record<string, unknown>;
};

export type QoinFeatureVector = {
  entity: QoinEntityRef;
  as_of_ms: number;
  model_id: string;
  model_version: string;
  features: Record<string, number>;
};

export type QoinDomainDecision = {
  entity: QoinEntityRef;
  decided_at_ms: number;
  algorithm_id: string;
  algorithm_version: string;
  action: string;
  confidence: number;
  reason_codes: string[];
  evidence_ids?: string[];
  attributes?: Record<string, unknown>;
};
