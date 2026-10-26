import torch
import pytest

from qoin_jepa import MarketJEPA, latent_prediction_loss


def test_predicts_future_latent_with_expected_shape() -> None:
    model = MarketJEPA(feature_dim=10, latent_dim=16, hidden_dim=32, max_horizon=4)
    context = torch.randn(3, 12, 10)
    future = torch.randn(3, 10)

    predicted, target = model(context, future, horizon=2)

    assert predicted.shape == (3, 16)
    assert target is not None
    assert target.shape == (3, 16)
    assert not target.requires_grad


def test_inference_does_not_require_future_observation() -> None:
    model = MarketJEPA(feature_dim=6, latent_dim=8, hidden_dim=16, max_horizon=3)
    predicted, target = model(torch.randn(2, 5, 6), horizon=1)

    assert predicted.shape == (2, 8)
    assert target is None


def test_rejects_invalid_horizon() -> None:
    model = MarketJEPA(feature_dim=6, latent_dim=8, hidden_dim=16, max_horizon=3)

    with pytest.raises(ValueError):
        model(torch.randn(2, 5, 6), horizon=4)


def test_loss_requires_matching_shapes() -> None:
    with pytest.raises(ValueError):
        latent_prediction_loss(torch.randn(2, 8), torch.randn(2, 7))


def test_target_encoder_is_updated_by_ema() -> None:
    model = MarketJEPA(feature_dim=4, latent_dim=8, hidden_dim=16)
    with torch.no_grad():
        for parameter in model.encoder.parameters():
            parameter.add_(0.1)
    before = [p.clone() for p in model.target_encoder.parameters()]
    model.update_target_encoder()
    after = list(model.target_encoder.parameters())

    assert any(not torch.equal(old, new) for old, new in zip(before, after, strict=True))
    assert all(not p.requires_grad for p in model.target_encoder.parameters())


def test_training_step_updates_model_and_returns_finite_loss() -> None:
    from qoin_jepa import train_batch

    model = MarketJEPA(feature_dim=5, latent_dim=8, hidden_dim=16, max_horizon=3)
    # Train the online encoder and predictor; the target encoder is EMA-only.
    optimizer = torch.optim.Adam(
        [parameter for parameter in model.parameters() if parameter.requires_grad],
        lr=1e-3,
    )
    loss = train_batch(
        model,
        optimizer,
        torch.randn(4, 6, 5),
        torch.randn(4, 5),
        horizon=2,
    )

    assert loss.ndim == 0
    assert torch.isfinite(loss)


def test_training_rejects_non_finite_input() -> None:
    from qoin_jepa import train_batch

    model = MarketJEPA(feature_dim=3, latent_dim=4, hidden_dim=8)
    optimizer = torch.optim.Adam(
        [parameter for parameter in model.parameters() if parameter.requires_grad],
        lr=1e-3,
    )
    context = torch.randn(2, 4, 3)
    context[0, 0, 0] = float("nan")

    with pytest.raises(ValueError):
        train_batch(model, optimizer, context, torch.randn(2, 3), horizon=1)
