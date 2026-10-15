package nines

import (
	"os"
	"testing"
)

// The override that did not work: a config file is shared by every machine, so
// it cannot be the last word on what one terminal looks like.
func TestEnvironmentBeatsAConfiguredAppearance(t *testing.T) {
	t.Setenv("NINES_THEME", "dark")
	if got := ThemeFromSetting("light").Appearance; got != AppearanceDark {
		t.Fatalf("config light + NINES_THEME=dark gave %v, want dark", got)
	}
	t.Setenv("NINES_THEME", "light")
	if got := ThemeFromSetting("dark").Appearance; got != AppearanceLight {
		t.Fatalf("config dark + NINES_THEME=light gave %v, want light", got)
	}
}

func TestEmptyEnvironmentVariableIsNotAnOverride(t *testing.T) {
	t.Setenv("NINES_THEME", "   ")
	if got := ThemeFromSetting("dark").Appearance; got != AppearanceDark {
		t.Fatalf("blank NINES_THEME overrode the config, gave %v", got)
	}
}

func withoutThemeEnv(t *testing.T) {
	t.Helper()
	if _, ok := os.LookupEnv("NINES_THEME"); ok {
		t.Setenv("NINES_THEME", "")
	}
}

// The point of the setting: a light terminal that never sets COLORFGBG can
// still ask for the light palette, which detection alone could never give it.
func TestConfiguredAppearanceBeatsDetection(t *testing.T) {
	withoutThemeEnv(t)
	if got := ThemeFromSetting("light").Appearance; got != AppearanceLight {
		t.Fatalf("light setting gave %v", got)
	}
	if got := ThemeFromSetting("dark").Appearance; got != AppearanceDark {
		t.Fatalf("dark setting gave %v", got)
	}
}

func TestSystemAndEmptyBothMeanWorkItOut(t *testing.T) {
	withoutThemeEnv(t)
	want := ThemeFromEnv().Appearance
	for _, setting := range []string{"", "  ", "system", "auto"} {
		if got := ThemeFromSetting(setting).Appearance; got != want {
			t.Fatalf("%q gave %v, want the detected %v", setting, got, want)
		}
	}
}

func TestNonsenseSettingFallsBackRatherThanFailing(t *testing.T) {
	withoutThemeEnv(t)
	if got := ThemeFromSetting("chartreuse").Appearance; got != ThemeFromEnv().Appearance {
		t.Fatalf("nonsense setting gave %v", got)
	}
}

// A configured appearance must still be legible where it matters most.
func TestEveryConfiguredAppearanceHasAReadableSelection(t *testing.T) {
	for _, setting := range []string{"light", "dark", "neutral"} {
		theme := ThemeFromSetting(setting)
		fg, bg, _ := theme.Selected().Decompose()
		if fg == bg {
			t.Fatalf("%s: selection is invisible, fg == bg == %v", setting, fg)
		}
	}
}
