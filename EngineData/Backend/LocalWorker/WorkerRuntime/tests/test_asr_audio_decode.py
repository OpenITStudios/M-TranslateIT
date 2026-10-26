"""Exercise the real faster-whisper/PyAV boundary without model downloads."""
import wave

import numpy as np
import pytest
from faster_whisper.audio import decode_audio


@pytest.mark.parametrize(("rate", "channels"), [(16000, 1), (48000, 2)])
def test_actual_decoder_accepts_public_pcm_wav(tmp_path, rate, channels):
    # Non-silent synthetic PCM covers finalized mono input and input resampling.
    signal = np.rint(np.sin(2 * np.pi * 440 * np.arange(rate) / rate) * 6000).astype("<i2")
    pcm = np.repeat(signal[:, None], channels, axis=1)
    audio = tmp_path / "public-tone.wav"
    with wave.open(str(audio), "wb") as output:
        output.setnchannels(channels)
        output.setsampwidth(2)
        output.setframerate(rate)
        output.writeframes(pcm.tobytes())

    decoded = decode_audio(str(audio), sampling_rate=16000)

    assert decoded.dtype == np.float32
    assert decoded.shape == (16000,)
    assert np.isfinite(decoded).all()
    assert 0.10 < float(np.sqrt(np.mean(decoded**2))) < 0.15
