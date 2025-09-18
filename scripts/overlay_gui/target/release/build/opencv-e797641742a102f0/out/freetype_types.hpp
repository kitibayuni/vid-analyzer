extern "C" {
	const cv::freetype::FreeType2* cv_PtrLcv_freetype_FreeType2G_getInnerPtr_const(const cv::Ptr<cv::freetype::FreeType2>* instance) {
			return instance->get();
	}

	cv::freetype::FreeType2* cv_PtrLcv_freetype_FreeType2G_getInnerPtrMut(cv::Ptr<cv::freetype::FreeType2>* instance) {
			return instance->get();
	}

	cv::Ptr<cv::freetype::FreeType2>* cv_PtrLcv_freetype_FreeType2G_new_null_const() {
			return new cv::Ptr<cv::freetype::FreeType2>();
	}

	void cv_PtrLcv_freetype_FreeType2G_delete(cv::Ptr<cv::freetype::FreeType2>* instance) {
			delete instance;
	}

	cv::Ptr<cv::Algorithm>* cv_PtrLcv_freetype_FreeType2G_to_PtrOfAlgorithm(cv::Ptr<cv::freetype::FreeType2>* instance) {
			return new cv::Ptr<cv::Algorithm>(instance->dynamicCast<cv::Algorithm>());
	}

}

